//! NIP-34 event builders for AgentGit
//!
//! Provides builder functions for creating NIP-34 git events and extensions
//! for agent-native workflows (issue claims, bounties, etc.).

use nostr::EventTemplate;
use std::time::{SystemTime, UNIX_EPOCH};

/// Builder for creating issue claim events (kind:1634)
///
/// An issue claim event allows an agent to claim an issue for work.
#[allow(dead_code)]
pub struct IssueClaimBuilder {
    issue_event_id: String,
    repo_address: String,
    issue_author_pubkey: String,
    content: Option<String>,
    trajectory_session_id: Option<String>,
    estimate_seconds: Option<u64>,
}

#[allow(dead_code)]
impl IssueClaimBuilder {
    /// Create a new issue claim builder
    ///
    /// # Arguments
    /// * `issue_event_id` - The event ID of the issue being claimed
    /// * `repo_address` - The repository address tag (e.g., "30617:<pubkey>:<repo-id>")
    /// * `issue_author_pubkey` - The pubkey of the issue author
    pub fn new(
        issue_event_id: impl Into<String>,
        repo_address: impl Into<String>,
        issue_author_pubkey: impl Into<String>,
    ) -> Self {
        Self {
            issue_event_id: issue_event_id.into(),
            repo_address: repo_address.into(),
            issue_author_pubkey: issue_author_pubkey.into(),
            content: None,
            trajectory_session_id: None,
            estimate_seconds: None,
        }
    }

    /// Set the content/message for the claim
    pub fn content(mut self, content: impl Into<String>) -> Self {
        self.content = Some(content.into());
        self
    }

    /// Set the trajectory session ID that will track the work
    pub fn trajectory(mut self, session_id: impl Into<String>) -> Self {
        self.trajectory_session_id = Some(session_id.into());
        self
    }

    /// Set the estimated completion time in seconds
    pub fn estimate(mut self, seconds: u64) -> Self {
        self.estimate_seconds = Some(seconds);
        self
    }

    /// Build the event template
    pub fn build(self) -> EventTemplate {
        let mut tags = vec![
            // Reference to issue (root marker)
            vec!["e".to_string(), self.issue_event_id, "".to_string(), "root".to_string()],
            // Repository reference
            vec!["a".to_string(), self.repo_address],
            // Issue author reference
            vec!["p".to_string(), self.issue_author_pubkey],
        ];

        // Add optional trajectory session ID
        if let Some(session_id) = self.trajectory_session_id {
            tags.push(vec!["trajectory".to_string(), session_id]);
        }

        // Add optional estimate
        if let Some(estimate) = self.estimate_seconds {
            tags.push(vec!["estimate".to_string(), estimate.to_string()]);
        }

        EventTemplate {
            created_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            kind: 1634, // Issue Claim
            tags,
            content: self.content.unwrap_or_default(),
        }
    }
}

/// Builder for creating bounty offer events (kind:1636)
///
/// A bounty offer event attaches a bitcoin bounty to an issue.
#[allow(dead_code)]
pub struct BountyOfferBuilder {
    issue_event_id: String,
    repo_address: String,
    amount_sats: u64,
    expiry_timestamp: Option<u64>,
    conditions: Vec<String>,
}

#[allow(dead_code)]
impl BountyOfferBuilder {
    /// Create a new bounty offer builder
    ///
    /// # Arguments
    /// * `issue_event_id` - The event ID of the issue
    /// * `repo_address` - The repository address tag (e.g., "30617:<pubkey>:<repo-id>")
    /// * `amount_sats` - The bounty amount in satoshis
    pub fn new(
        issue_event_id: impl Into<String>,
        repo_address: impl Into<String>,
        amount_sats: u64,
    ) -> Self {
        Self {
            issue_event_id: issue_event_id.into(),
            repo_address: repo_address.into(),
            amount_sats,
            expiry_timestamp: None,
            conditions: Vec::new(),
        }
    }

    /// Set the expiry timestamp (Unix timestamp in seconds)
    pub fn expiry(mut self, timestamp: u64) -> Self {
        self.expiry_timestamp = Some(timestamp);
        self
    }

    /// Add a condition for claiming the bounty
    pub fn condition(mut self, condition: impl Into<String>) -> Self {
        self.conditions.push(condition.into());
        self
    }

    /// Build the event template
    pub fn build(self) -> EventTemplate {
        let mut tags = vec![
            // Reference to issue (root marker)
            vec!["e".to_string(), self.issue_event_id, "".to_string(), "root".to_string()],
            // Repository reference
            vec!["a".to_string(), self.repo_address],
            // Amount in sats
            vec!["amount".to_string(), self.amount_sats.to_string()],
        ];

        // Add optional expiry
        if let Some(expiry) = self.expiry_timestamp {
            tags.push(vec!["expiry".to_string(), expiry.to_string()]);
        }

        // Add conditions
        for condition in self.conditions {
            tags.push(vec!["conditions".to_string(), condition]);
        }

        EventTemplate {
            created_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            kind: 1636, // Bounty Offer
            tags,
            content: String::new(),
        }
    }
}

/// Builder for creating work assignment events (kind:1635)
///
/// A work assignment event allows maintainers to assign issues to specific agents.
#[allow(dead_code)]
pub struct WorkAssignmentBuilder {
    issue_event_id: String,
    repo_address: String,
    assignee_pubkey: String,
    content: Option<String>,
}

#[allow(dead_code)]
impl WorkAssignmentBuilder {
    /// Create a new work assignment builder
    ///
    /// # Arguments
    /// * `issue_event_id` - The event ID of the issue being assigned
    /// * `repo_address` - The repository address tag (e.g., "30617:<pubkey>:<repo-id>")
    /// * `assignee_pubkey` - The pubkey of the agent being assigned
    pub fn new(
        issue_event_id: impl Into<String>,
        repo_address: impl Into<String>,
        assignee_pubkey: impl Into<String>,
    ) -> Self {
        Self {
            issue_event_id: issue_event_id.into(),
            repo_address: repo_address.into(),
            assignee_pubkey: assignee_pubkey.into(),
            content: None,
        }
    }

    /// Set the content/message for the assignment
    pub fn content(mut self, content: impl Into<String>) -> Self {
        self.content = Some(content.into());
        self
    }

    /// Build the event template
    pub fn build(self) -> EventTemplate {
        let tags = vec![
            // Reference to issue (root marker)
            vec!["e".to_string(), self.issue_event_id, "".to_string(), "root".to_string()],
            // Repository reference
            vec!["a".to_string(), self.repo_address],
            // Assignee reference
            vec![
                "p".to_string(),
                self.assignee_pubkey,
                "".to_string(),
                "assignee".to_string(),
            ],
        ];

        EventTemplate {
            created_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            kind: 1635, // Work Assignment
            tags,
            content: self.content.unwrap_or_default(),
        }
    }
}

/// Builder for creating bounty claim events (kind:1637)
///
/// A bounty claim event is created when work is completed and the agent
/// is claiming payment for a bounty.
#[allow(dead_code)]
pub struct BountyClaimBuilder {
    bounty_event_id: String,
    merged_pr_event_id: String,
    repo_address: String,
    trajectory_session_id: String,
    trajectory_hash: String,
    lightning_address: Option<String>,
    relay_hint: Option<String>,
}

#[allow(dead_code)]
impl BountyClaimBuilder {
    /// Create a new bounty claim builder
    ///
    /// # Arguments
    /// * `bounty_event_id` - The event ID of the bounty offer
    /// * `merged_pr_event_id` - The event ID of the merged PR
    /// * `repo_address` - The repository address tag
    /// * `trajectory_session_id` - The trajectory session ID proving the work
    /// * `trajectory_hash` - SHA256 hash of all trajectory events
    pub fn new(
        bounty_event_id: impl Into<String>,
        merged_pr_event_id: impl Into<String>,
        repo_address: impl Into<String>,
        trajectory_session_id: impl Into<String>,
        trajectory_hash: impl Into<String>,
    ) -> Self {
        Self {
            bounty_event_id: bounty_event_id.into(),
            merged_pr_event_id: merged_pr_event_id.into(),
            repo_address: repo_address.into(),
            trajectory_session_id: trajectory_session_id.into(),
            trajectory_hash: trajectory_hash.into(),
            lightning_address: None,
            relay_hint: None,
        }
    }

    /// Set the Lightning address (lud16) for payment
    pub fn lightning_address(mut self, address: impl Into<String>) -> Self {
        self.lightning_address = Some(address.into());
        self
    }

    /// Set the relay hint for finding trajectory events
    pub fn relay(mut self, relay_url: impl Into<String>) -> Self {
        self.relay_hint = Some(relay_url.into());
        self
    }

    /// Build the event template
    pub fn build(self) -> EventTemplate {
        let mut tags = vec![
            // Reference to bounty (root marker)
            vec!["e".to_string(), self.bounty_event_id, "".to_string(), "root".to_string()],
            // Reference to merged PR
            vec!["e".to_string(), self.merged_pr_event_id, "".to_string(), "mention".to_string()],
            // Repository reference
            vec!["a".to_string(), self.repo_address],
        ];

        // Add trajectory with optional relay hint
        if let Some(relay) = &self.relay_hint {
            tags.push(vec!["trajectory".to_string(), self.trajectory_session_id.clone(), relay.clone()]);
        } else {
            tags.push(vec!["trajectory".to_string(), self.trajectory_session_id.clone()]);
        }

        // Add trajectory hash
        tags.push(vec!["trajectory_hash".to_string(), self.trajectory_hash]);

        // Add optional lightning address
        if let Some(lud16) = self.lightning_address {
            tags.push(vec!["lud16".to_string(), lud16]);
        }

        EventTemplate {
            created_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            kind: 1637, // Bounty Claim
            tags,
            content: String::new(),
        }
    }
}

/// Builder for creating status events (kinds 1630-1633)
///
/// Status events are used to mark PRs/patches as:
/// - 1630: Open
/// - 1631: Applied/Merged
/// - 1632: Closed
/// - 1633: Draft
#[allow(dead_code)]
pub struct StatusEventBuilder {
    target_event_id: String,
    repo_address: String,
    status_kind: u16,
    reason: Option<String>,
}

#[allow(dead_code)]
impl StatusEventBuilder {
    /// Create a new status event builder
    ///
    /// # Arguments
    /// * `target_event_id` - The event ID of the PR/patch being updated
    /// * `repo_address` - The repository address tag (e.g., "30617:<pubkey>:<repo-id>")
    /// * `status_kind` - The status kind (1630=Open, 1631=Applied, 1632=Closed, 1633=Draft)
    pub fn new(
        target_event_id: impl Into<String>,
        repo_address: impl Into<String>,
        status_kind: u16,
    ) -> Self {
        Self {
            target_event_id: target_event_id.into(),
            repo_address: repo_address.into(),
            status_kind,
            reason: None,
        }
    }

    /// Create a status event for marking as Open (1630)
    pub fn open(target_event_id: impl Into<String>, repo_address: impl Into<String>) -> Self {
        Self::new(target_event_id, repo_address, 1630)
    }

    /// Create a status event for marking as Applied/Merged (1631)
    pub fn applied(target_event_id: impl Into<String>, repo_address: impl Into<String>) -> Self {
        Self::new(target_event_id, repo_address, 1631)
    }

    /// Create a status event for marking as Closed (1632)
    pub fn closed(target_event_id: impl Into<String>, repo_address: impl Into<String>) -> Self {
        Self::new(target_event_id, repo_address, 1632)
    }

    /// Create a status event for marking as Draft (1633)
    pub fn draft(target_event_id: impl Into<String>, repo_address: impl Into<String>) -> Self {
        Self::new(target_event_id, repo_address, 1633)
    }

    /// Set an optional reason/message for the status change
    pub fn reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = Some(reason.into());
        self
    }

    /// Build the event template
    pub fn build(self) -> EventTemplate {
        let tags = vec![
            // Reference to target PR/patch (root marker)
            vec!["e".to_string(), self.target_event_id, "".to_string(), "root".to_string()],
            // Repository reference
            vec!["a".to_string(), self.repo_address],
        ];

        EventTemplate {
            created_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            kind: self.status_kind,
            tags,
            content: self.reason.unwrap_or_default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_issue_claim_builder() {
        let template = IssueClaimBuilder::new(
            "issue123",
            "30617:pubkey123:repo456",
            "author789",
        )
        .content("I'll work on this. Estimated completion: 2 hours.")
        .trajectory("session_abc")
        .estimate(7200)
        .build();

        assert_eq!(template.kind, 1634);
        assert!(template.tags.iter().any(|t| t[0] == "e" && t[1] == "issue123"));
        assert!(template.tags.iter().any(|t| t[0] == "a" && t[1] == "30617:pubkey123:repo456"));
        assert!(template.tags.iter().any(|t| t[0] == "p" && t[1] == "author789"));
        assert!(template.tags.iter().any(|t| t[0] == "trajectory" && t[1] == "session_abc"));
        assert!(template.tags.iter().any(|t| t[0] == "estimate" && t[1] == "7200"));
    }

    #[test]
    fn test_bounty_offer_builder() {
        let template = BountyOfferBuilder::new(
            "issue123",
            "30617:pubkey123:repo456",
            50000,
        )
        .expiry(1700000000)
        .condition("must include tests")
        .condition("must pass CI")
        .build();

        assert_eq!(template.kind, 1636);
        assert!(template.tags.iter().any(|t| t[0] == "e" && t[1] == "issue123"));
        assert!(template.tags.iter().any(|t| t[0] == "amount" && t[1] == "50000"));
        assert!(template.tags.iter().any(|t| t[0] == "expiry" && t[1] == "1700000000"));
        assert!(template.tags.iter().filter(|t| t[0] == "conditions").count() == 2);
    }

    #[test]
    fn test_work_assignment_builder() {
        let template = WorkAssignmentBuilder::new(
            "issue123",
            "30617:pubkey123:repo456",
            "agent789",
        )
        .content("Assigned to @agent")
        .build();

        assert_eq!(template.kind, 1635);
        assert!(template.tags.iter().any(|t| t[0] == "p" && t[1] == "agent789" && t.get(3) == Some(&"assignee".to_string())));
    }

    #[test]
    fn test_bounty_claim_builder() {
        let template = BountyClaimBuilder::new(
            "bounty123",
            "pr456",
            "30617:pubkey123:repo456",
            "session_abc",
            "hash_def",
        )
        .lightning_address("agent@getalby.com")
        .relay("wss://relay.nostr.bg")
        .build();

        assert_eq!(template.kind, 1637);
        assert!(template.tags.iter().any(|t| t[0] == "trajectory_hash" && t[1] == "hash_def"));
        assert!(template.tags.iter().any(|t| t[0] == "lud16" && t[1] == "agent@getalby.com"));
    }
}
