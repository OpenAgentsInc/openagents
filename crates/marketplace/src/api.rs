//! Marketplace API route types
//!
//! This module defines request and response types for all marketplace API endpoints
//! including skills, compute, agents, payments, and data APIs.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Errors that can occur during API operations
#[derive(Debug, Clone, Error, PartialEq, Eq, Serialize, Deserialize)]
pub enum ApiError {
    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Unauthorized: {0}")]
    Unauthorized(String),

    #[error("Invalid request: {0}")]
    InvalidRequest(String),

    #[error("Internal error: {0}")]
    InternalError(String),

    #[error("Rate limit exceeded")]
    RateLimitExceeded,
}

// ============================================================================
// Skills API
// ============================================================================

/// Query parameters for listing skills
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillQuery {
    /// Search term
    pub search: Option<String>,

    /// Filter by category
    pub category: Option<String>,

    /// Sort order
    pub sort: Option<String>,

    /// Page number
    pub page: Option<u32>,

    /// Items per page
    pub limit: Option<u32>,
}

impl SkillQuery {
    /// Create a new skill query
    pub fn new() -> Self {
        Self {
            search: None,
            category: None,
            sort: None,
            page: None,
            limit: None,
        }
    }

    /// Set search term
    pub fn with_search(mut self, search: impl Into<String>) -> Self {
        self.search = Some(search.into());
        self
    }

    /// Set category filter
    pub fn with_category(mut self, category: impl Into<String>) -> Self {
        self.category = Some(category.into());
        self
    }
}

impl Default for SkillQuery {
    fn default() -> Self {
        Self::new()
    }
}

/// Request to submit a skill
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillSubmissionRequest {
    /// Skill name
    pub name: String,

    /// Skill description
    pub description: String,

    /// Skill code/content
    pub content: String,

    /// Author public key
    pub author: String,

    /// Categories
    pub categories: Vec<String>,
}

/// Response after skill submission
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillSubmissionResponse {
    /// Skill ID
    pub skill_id: String,

    /// Submission status
    pub status: String,

    /// Review URL
    pub review_url: String,

    /// Submitted at
    pub submitted_at: DateTime<Utc>,
}

/// Request to install a skill
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillInstallRequest {
    /// Skill ID to install
    pub skill_id: String,

    /// Optional version
    pub version: Option<String>,
}

/// Response after skill installation
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillInstallResponse {
    /// Installation ID
    pub installation_id: String,

    /// Installed skill ID
    pub skill_id: String,

    /// Installed version
    pub version: String,

    /// Installation status
    pub status: String,

    /// Installed at
    pub installed_at: DateTime<Utc>,
}

// ============================================================================
// Compute API
// ============================================================================

/// Request to register as a compute provider
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProviderRegistrationRequest {
    /// Provider name
    pub name: String,

    /// Provider public key
    pub pubkey: String,

    /// Supported models
    pub models: Vec<String>,

    /// Price per million tokens (satoshis)
    pub price_per_million_tokens: u64,

    /// Region
    pub region: Option<String>,

    /// Capabilities
    pub capabilities: Vec<String>,
}

/// Response after provider registration
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderRegistrationResponse {
    /// Provider ID
    pub provider_id: String,

    /// Registration status
    pub status: String,

    /// Registered at
    pub registered_at: DateTime<Utc>,

    /// API endpoint
    pub endpoint: String,
}

/// Request to submit a compute job
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ComputeJobRequest {
    /// Model to use
    pub model: String,

    /// Prompt/input
    pub prompt: String,

    /// Max tokens
    pub max_tokens: Option<u32>,

    /// Temperature
    pub temperature: Option<f32>,

    /// Preferred providers
    pub preferred_providers: Option<Vec<String>>,
}

/// Response with compute job result
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ComputeJobResponse {
    /// Job ID
    pub job_id: String,

    /// Job status
    pub status: String,

    /// Provider that executed the job
    pub provider_id: String,

    /// Result (if completed)
    pub result: Option<String>,

    /// Tokens used
    pub tokens_used: Option<u32>,

    /// Cost in satoshis
    pub cost_sats: Option<u64>,

    /// Created at
    pub created_at: DateTime<Utc>,

    /// Completed at
    pub completed_at: Option<DateTime<Utc>>,
}

// ============================================================================
// Agents API
// ============================================================================

/// Request to spawn an agent
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentSpawnRequest {
    /// Agent name
    pub name: String,

    /// Agent description
    pub description: String,

    /// Skills to equip
    pub skills: Vec<String>,

    /// Initial budget in satoshis
    pub budget_sats: Option<u64>,

    /// Owner public key
    pub owner: String,
}

/// Response after spawning agent
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentSpawnResponse {
    /// Agent ID
    pub agent_id: String,

    /// Agent public key
    pub pubkey: String,

    /// Status
    pub status: String,

    /// Spawned at
    pub spawned_at: DateTime<Utc>,
}

/// Request to hire an agent
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HireAgentRequest {
    /// Agent ID to hire
    pub agent_id: String,

    /// Task description
    pub task: String,

    /// Budget for task
    pub budget_sats: u64,

    /// Deadline
    pub deadline: Option<DateTime<Utc>>,
}

/// Response after hiring agent
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HireAgentResponse {
    /// Contract ID
    pub contract_id: String,

    /// Agent ID
    pub agent_id: String,

    /// Task description
    pub task: String,

    /// Budget
    pub budget_sats: u64,

    /// Contract status
    pub status: String,

    /// Created at
    pub created_at: DateTime<Utc>,
}

/// Request to propose a coalition
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoalitionProposalRequest {
    /// Coalition name
    pub name: String,

    /// Member agent IDs
    pub members: Vec<String>,

    /// Goal/purpose
    pub goal: String,

    /// Revenue split
    pub revenue_split: Vec<(String, u8)>, // (agent_id, percentage)
}

/// Response after coalition proposal
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoalitionProposalResponse {
    /// Coalition ID
    pub coalition_id: String,

    /// Proposal status
    pub status: String,

    /// Members who accepted
    pub accepted_by: Vec<String>,

    /// Created at
    pub created_at: DateTime<Utc>,
}

// ============================================================================
// Payments API
// ============================================================================

/// Request to create an invoice
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InvoiceRequest {
    /// Amount in satoshis
    pub amount_sats: u64,

    /// Description
    pub description: String,

    /// Expiry in seconds
    pub expiry_seconds: Option<u64>,
}

/// Response with lightning invoice
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InvoiceResponse {
    /// Invoice ID
    pub invoice_id: String,

    /// Payment request (BOLT11)
    pub payment_request: String,

    /// Amount
    pub amount_sats: u64,

    /// Payment hash
    pub payment_hash: String,

    /// Status
    pub status: String,

    /// Expires at
    pub expires_at: DateTime<Utc>,

    /// Created at
    pub created_at: DateTime<Utc>,
}

/// Request to send a payment
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PaymentRequest {
    /// Payment request (BOLT11) or destination
    pub destination: String,

    /// Amount (if not in invoice)
    pub amount_sats: Option<u64>,

    /// Maximum fee willing to pay
    pub max_fee_sats: Option<u64>,
}

/// Response after sending payment
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PaymentResponse {
    /// Payment ID
    pub payment_id: String,

    /// Payment hash
    pub payment_hash: String,

    /// Amount sent
    pub amount_sats: u64,

    /// Fee paid
    pub fee_sats: u64,

    /// Status
    pub status: String,

    /// Sent at
    pub sent_at: DateTime<Utc>,
}

/// Response with account balance
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BalanceResponse {
    /// Account ID
    pub account_id: String,

    /// Available balance
    pub available_sats: u64,

    /// Pending balance
    pub pending_sats: u64,

    /// Total balance
    pub total_sats: u64,

    /// Last updated
    pub updated_at: DateTime<Utc>,
}

/// Query parameters for ledger
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LedgerQuery {
    /// Account ID
    pub account_id: String,

    /// Start date
    pub start_date: Option<DateTime<Utc>>,

    /// End date
    pub end_date: Option<DateTime<Utc>>,

    /// Entry type filter
    pub entry_type: Option<String>,

    /// Page number
    pub page: Option<u32>,

    /// Items per page
    pub limit: Option<u32>,
}

// ============================================================================
// Data API
// ============================================================================

/// Request to contribute data
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DataContributionRequest {
    /// Data type
    pub data_type: String,

    /// Content hash
    pub content_hash: String,

    /// Size in bytes
    pub size_bytes: u64,

    /// Description
    pub description: String,

    /// Tags
    pub tags: Vec<String>,

    /// License
    pub license: String,
}

/// Response after data contribution
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DataContributionResponse {
    /// Contribution ID
    pub contribution_id: String,

    /// Status
    pub status: String,

    /// Upload URL (if approved)
    pub upload_url: Option<String>,

    /// Submitted at
    pub submitted_at: DateTime<Utc>,
}

/// Query parameters for data listings
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DataListingsQuery {
    /// Data type filter
    pub data_type: Option<String>,

    /// Search term
    pub search: Option<String>,

    /// Tags
    pub tags: Option<Vec<String>>,

    /// Min price
    pub min_price_sats: Option<u64>,

    /// Max price
    pub max_price_sats: Option<u64>,

    /// Page number
    pub page: Option<u32>,

    /// Items per page
    pub limit: Option<u32>,
}

/// Request to purchase data
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DataPurchaseRequest {
    /// Listing ID
    pub listing_id: String,

    /// Access duration (hours)
    pub access_duration_hours: Option<u32>,
}

/// Response after data purchase
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DataPurchaseResponse {
    /// Purchase ID
    pub purchase_id: String,

    /// Listing ID
    pub listing_id: String,

    /// Access token
    pub access_token: String,

    /// Download URL
    pub download_url: String,

    /// Amount paid
    pub amount_sats: u64,

    /// Expires at
    pub expires_at: Option<DateTime<Utc>>,

    /// Purchased at
    pub purchased_at: DateTime<Utc>,
}

/// Request to create a data bounty
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DataBountyRequest {
    /// Bounty title
    pub title: String,

    /// Description of needed data
    pub description: String,

    /// Data type
    pub data_type: String,

    /// Requirements
    pub requirements: Vec<String>,

    /// Bounty amount
    pub bounty_sats: u64,

    /// Deadline
    pub deadline: Option<DateTime<Utc>>,
}

/// Response after creating bounty
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DataBountyResponse {
    /// Bounty ID
    pub bounty_id: String,

    /// Title
    pub title: String,

    /// Amount
    pub bounty_sats: u64,

    /// Status
    pub status: String,

    /// Created at
    pub created_at: DateTime<Utc>,

    /// Deadline
    pub deadline: Option<DateTime<Utc>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_skill_query_builder() {
        let query = SkillQuery::new()
            .with_search("image processing")
            .with_category("vision");

        assert_eq!(query.search, Some("image processing".to_string()));
        assert_eq!(query.category, Some("vision".to_string()));
    }

    #[test]
    fn test_skill_query_default() {
        let query = SkillQuery::default();
        assert!(query.search.is_none());
        assert!(query.category.is_none());
    }

    #[test]
    fn test_skill_submission_request() {
        let request = SkillSubmissionRequest {
            name: "test-skill".to_string(),
            description: "Test skill".to_string(),
            content: "code here".to_string(),
            author: "pubkey123".to_string(),
            categories: vec!["test".to_string()],
        };

        assert_eq!(request.name, "test-skill");
        assert_eq!(request.categories.len(), 1);
    }

    #[test]
    fn test_skill_install_request() {
        let request = SkillInstallRequest {
            skill_id: "skill1".to_string(),
            version: Some("1.0.0".to_string()),
        };

        assert_eq!(request.skill_id, "skill1");
        assert_eq!(request.version, Some("1.0.0".to_string()));
    }

    #[test]
    fn test_provider_registration_request() {
        let request = ProviderRegistrationRequest {
            name: "provider1".to_string(),
            pubkey: "pubkey123".to_string(),
            models: vec!["gpt-4".to_string()],
            price_per_million_tokens: 1000,
            region: Some("us-east".to_string()),
            capabilities: vec!["gpu".to_string()],
        };

        assert_eq!(request.models.len(), 1);
        assert_eq!(request.price_per_million_tokens, 1000);
    }

    #[test]
    fn test_compute_job_request() {
        let request = ComputeJobRequest {
            model: "gpt-4".to_string(),
            prompt: "Hello".to_string(),
            max_tokens: Some(100),
            temperature: Some(0.7),
            preferred_providers: None,
        };

        assert_eq!(request.model, "gpt-4");
        assert_eq!(request.max_tokens, Some(100));
    }

    #[test]
    fn test_agent_spawn_request() {
        let request = AgentSpawnRequest {
            name: "agent1".to_string(),
            description: "Test agent".to_string(),
            skills: vec!["skill1".to_string()],
            budget_sats: Some(10000),
            owner: "owner123".to_string(),
        };

        assert_eq!(request.skills.len(), 1);
        assert_eq!(request.budget_sats, Some(10000));
    }

    #[test]
    fn test_hire_agent_request() {
        let request = HireAgentRequest {
            agent_id: "agent1".to_string(),
            task: "Do work".to_string(),
            budget_sats: 5000,
            deadline: None,
        };

        assert_eq!(request.budget_sats, 5000);
        assert!(request.deadline.is_none());
    }

    #[test]
    fn test_coalition_proposal_request() {
        let request = CoalitionProposalRequest {
            name: "coalition1".to_string(),
            members: vec!["agent1".to_string(), "agent2".to_string()],
            goal: "Collaborate".to_string(),
            revenue_split: vec![("agent1".to_string(), 50), ("agent2".to_string(), 50)],
        };

        assert_eq!(request.members.len(), 2);
        assert_eq!(request.revenue_split.len(), 2);
    }

    #[test]
    fn test_invoice_request() {
        let request = InvoiceRequest {
            amount_sats: 1000,
            description: "Payment for service".to_string(),
            expiry_seconds: Some(3600),
        };

        assert_eq!(request.amount_sats, 1000);
        assert_eq!(request.expiry_seconds, Some(3600));
    }

    #[test]
    fn test_payment_request() {
        let request = PaymentRequest {
            destination: "lnbc...".to_string(),
            amount_sats: Some(500),
            max_fee_sats: Some(10),
        };

        assert_eq!(request.amount_sats, Some(500));
        assert_eq!(request.max_fee_sats, Some(10));
    }

    #[test]
    fn test_balance_response() {
        let now = Utc::now();
        let response = BalanceResponse {
            account_id: "account1".to_string(),
            available_sats: 10000,
            pending_sats: 1000,
            total_sats: 11000,
            updated_at: now,
        };

        assert_eq!(response.total_sats, 11000);
        assert_eq!(
            response.available_sats + response.pending_sats,
            response.total_sats
        );
    }

    #[test]
    fn test_data_contribution_request() {
        let request = DataContributionRequest {
            data_type: "training-data".to_string(),
            content_hash: "abc123".to_string(),
            size_bytes: 1024000,
            description: "Dataset".to_string(),
            tags: vec!["nlp".to_string()],
            license: "MIT".to_string(),
        };

        assert_eq!(request.size_bytes, 1024000);
        assert_eq!(request.tags.len(), 1);
    }

    #[test]
    fn test_data_purchase_request() {
        let request = DataPurchaseRequest {
            listing_id: "listing1".to_string(),
            access_duration_hours: Some(24),
        };

        assert_eq!(request.listing_id, "listing1");
        assert_eq!(request.access_duration_hours, Some(24));
    }

    #[test]
    fn test_data_bounty_request() {
        let request = DataBountyRequest {
            title: "Need dataset".to_string(),
            description: "Looking for X".to_string(),
            data_type: "images".to_string(),
            requirements: vec!["high quality".to_string()],
            bounty_sats: 100000,
            deadline: None,
        };

        assert_eq!(request.bounty_sats, 100000);
        assert_eq!(request.requirements.len(), 1);
    }

    #[test]
    fn test_skill_query_serde() {
        let query = SkillQuery::new().with_search("test");
        let json = serde_json::to_string(&query).unwrap();
        let deserialized: SkillQuery = serde_json::from_str(&json).unwrap();
        assert_eq!(query, deserialized);
    }

    #[test]
    fn test_compute_job_request_serde() {
        let request = ComputeJobRequest {
            model: "gpt-4".to_string(),
            prompt: "Hello".to_string(),
            max_tokens: Some(100),
            temperature: Some(0.7),
            preferred_providers: None,
        };

        let json = serde_json::to_string(&request).unwrap();
        let deserialized: ComputeJobRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(request.model, deserialized.model);
    }

    #[test]
    fn test_invoice_request_serde() {
        let request = InvoiceRequest {
            amount_sats: 1000,
            description: "Test".to_string(),
            expiry_seconds: Some(3600),
        };

        let json = serde_json::to_string(&request).unwrap();
        let deserialized: InvoiceRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(request, deserialized);
    }
}
