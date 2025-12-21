//! Trajectory contribution to marketplace via Nostr

use super::TrajectorySession;
use anyhow::Result;
use serde::{Deserialize, Serialize};

/// Status of a trajectory contribution
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContributionStatus {
    /// Pending review
    Pending,
    /// Accepted and paid
    Accepted,
    /// Rejected
    Rejected,
}

/// Request to contribute a trajectory
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContributionRequest {
    /// Session to contribute
    pub session: TrajectorySession,

    /// Redacted and anonymized content
    pub content: String,

    /// Hash of the trajectory for verification
    pub trajectory_hash: String,

    /// Lightning address for payment
    pub lightning_address: String,
}

/// Response from contribution submission
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContributionResponse {
    /// Contribution ID
    pub contribution_id: String,

    /// Initial status
    pub status: ContributionStatus,

    /// Estimated reward in sats (not guaranteed)
    pub estimated_reward_sats: u64,

    /// Message from marketplace
    pub message: String,
}

/// Client for contributing trajectories to the marketplace
pub struct ContributionClient {
    /// Relays to publish to
    _relays: Vec<String>,

    /// Identity for signing events
    identity: Option<String>, // Would be actual Nostr identity
}

impl ContributionClient {
    /// Create a new contribution client
    pub fn new(relays: Vec<String>) -> Self {
        Self {
            _relays: relays,
            identity: None,
        }
    }

    /// Set the identity for signing
    pub fn with_identity(mut self, identity: String) -> Self {
        self.identity = Some(identity);
        self
    }

    /// Submit a trajectory contribution
    pub async fn submit(&self, _request: ContributionRequest) -> Result<ContributionResponse> {
        // In a real implementation, this would:
        // 1. Create a Nostr event with the trajectory data
        // 2. Sign it with the user's identity
        // 3. Publish to configured relays
        // 4. Wait for marketplace acknowledgment

        // For now, return a mock response
        Ok(ContributionResponse {
            contribution_id: uuid::Uuid::new_v4().to_string(),
            status: ContributionStatus::Pending,
            estimated_reward_sats: 500, // Would be calculated by marketplace
            message: "Contribution received and pending review".to_string(),
        })
    }

    /// Check status of a contribution
    pub async fn check_status(&self, _contribution_id: &str) -> Result<ContributionStatus> {
        // In a real implementation, this would:
        // 1. Query relays for status events
        // 2. Find matching contribution_id
        // 3. Return current status

        // Mock response
        Ok(ContributionStatus::Pending)
    }

    /// Get contribution earnings
    pub async fn get_earnings(&self) -> Result<Vec<ContributionEarning>> {
        // In a real implementation, this would:
        // 1. Query for all accepted contributions
        // 2. Aggregate earnings by time period
        // 3. Return detailed breakdown

        Ok(Vec::new())
    }
}

/// Earning record for a contribution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContributionEarning {
    /// Contribution ID
    pub contribution_id: String,

    /// Session ID
    pub session_id: String,

    /// Reward amount in sats
    pub reward_sats: u64,

    /// When payment was received
    pub paid_at: chrono::DateTime<chrono::Utc>,

    /// Payment preimage (proof of payment)
    pub payment_preimage: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn test_contribution_request_creation() {
        let session = TrajectorySession {
            session_id: "test-123".to_string(),
            source: "claude".to_string(),
            path: "/tmp/test.rlog".into(),
            initial_commit: Some("abc".to_string()),
            final_commit: Some("def".to_string()),
            ci_passed: Some(true),
            started_at: Utc::now(),
            ended_at: Some(Utc::now()),
            token_count: 1000,
            tool_calls: 10,
            quality_score: 0.8,
        };

        let request = ContributionRequest {
            session,
            content: "redacted content".to_string(),
            trajectory_hash: "abc123hash".to_string(),
            lightning_address: "user@getalby.com".to_string(),
        };

        assert_eq!(request.session.session_id, "test-123");
        assert_eq!(request.lightning_address, "user@getalby.com");
    }
}
