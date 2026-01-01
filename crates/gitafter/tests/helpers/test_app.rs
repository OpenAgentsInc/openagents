//! Test helper for GitAfter integration tests
//!
//! Provides TestApp pattern for isolated testing with mock relays and test identities.

use anyhow::Result;
use gitafter::{NostrClient, WsBroadcaster};
use nostr::{EventTemplate, finalize_event, generate_secret_key};
use std::sync::Arc;
use testing::MockRelay;

/// Test application helper for GitAfter integration tests
#[allow(dead_code)]
pub struct TestApp {
    /// Mock Nostr relay for testing
    pub relay: MockRelay,
    /// Nostr client connected to mock relay
    pub client: NostrClient,
    /// Test identity secret key (32-byte array)
    pub secret_key: [u8; 32],
    /// WebSocket broadcaster for UI updates
    pub broadcaster: Arc<WsBroadcaster>,
}

#[allow(dead_code)]
impl TestApp {
    /// Create a new test application with mock relay
    pub async fn new() -> Result<Self> {
        // Start mock relay on unique port
        let relay = MockRelay::start().await;

        // Create broadcaster
        let broadcaster = Arc::new(WsBroadcaster::new(64));

        // Create client connected to mock relay
        let relay_url = relay.url().to_string();
        let client = NostrClient::new(vec![relay_url], broadcaster.clone())?;

        // Generate test identity
        let secret_key = generate_secret_key();

        Ok(Self {
            relay,
            client,
            secret_key,
            broadcaster,
        })
    }

    /// Get public key hex for test identity
    pub fn pubkey(&self) -> String {
        nostr::get_public_key_hex(&self.secret_key).expect("valid pubkey")
    }

    /// Publish an event to the mock relay
    pub async fn publish_event(&self, template: EventTemplate) -> Result<nostr::Event> {
        let event = finalize_event(&template, &self.secret_key)?;

        // Store event in relay
        self.relay.store_event(event.clone()).await;

        Ok(event)
    }

    /// Create a repository announcement (kind 30617)
    pub async fn create_repository(
        &self,
        identifier: &str,
        name: &str,
        description: &str,
    ) -> Result<nostr::Event> {
        let template = EventTemplate {
            kind: 30617, // REPOSITORY_ANNOUNCEMENT
            tags: vec![
                vec!["d".to_string(), identifier.to_string()],
                vec!["name".to_string(), name.to_string()],
                vec!["description".to_string(), description.to_string()],
            ],
            content: String::new(),
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        };

        self.publish_event(template).await
    }

    /// Create an issue (kind 1621)
    pub async fn create_issue(
        &self,
        repo_identifier: &str,
        title: &str,
        body: &str,
    ) -> Result<nostr::Event> {
        let template = EventTemplate {
            kind: 1621, // ISSUE
            tags: vec![
                vec![
                    "a".to_string(),
                    format!("30617:{}:{}", self.pubkey(), repo_identifier),
                ],
                vec!["subject".to_string(), title.to_string()],
            ],
            content: body.to_string(),
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        };

        self.publish_event(template).await
    }

    /// Claim an issue (kind 1634)
    pub async fn claim_issue(&self, issue_id: &str) -> Result<nostr::Event> {
        let template = EventTemplate {
            kind: 1634, // ISSUE_CLAIM
            tags: vec![vec![
                "e".to_string(),
                issue_id.to_string(),
                "".to_string(),
                "root".to_string(),
            ]],
            content: "Claiming this issue".to_string(),
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        };

        self.publish_event(template).await
    }

    /// Post a comment on an issue (kind 1)
    pub async fn comment_on_issue(&self, issue_id: &str, comment: &str) -> Result<nostr::Event> {
        let template = EventTemplate {
            kind: 1, // Short text note
            tags: vec![vec![
                "e".to_string(),
                issue_id.to_string(),
                "".to_string(),
                "root".to_string(),
            ]],
            content: comment.to_string(),
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        };

        self.publish_event(template).await
    }

    /// Create a bounty offer (kind 1636)
    pub async fn create_bounty(&self, issue_id: &str, amount_sats: u64) -> Result<nostr::Event> {
        let template = EventTemplate {
            kind: 1636, // BOUNTY_OFFER
            tags: vec![
                vec![
                    "e".to_string(),
                    issue_id.to_string(),
                    "".to_string(),
                    "root".to_string(),
                ],
                vec!["amount".to_string(), amount_sats.to_string()],
            ],
            content: String::new(),
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        };

        self.publish_event(template).await
    }

    /// Create a pull request (kind 1618)
    pub async fn create_pr(
        &self,
        repo_identifier: &str,
        title: &str,
        commit_id: &str,
        clone_url: &str,
        trajectory_session_id: Option<&str>,
    ) -> Result<nostr::Event> {
        let mut tags = vec![
            vec![
                "a".to_string(),
                format!("30617:{}:{}", self.pubkey(), repo_identifier),
            ],
            vec!["subject".to_string(), title.to_string()],
            vec!["c".to_string(), commit_id.to_string()],
            vec!["clone".to_string(), clone_url.to_string()],
        ];

        if let Some(traj_id) = trajectory_session_id {
            tags.push(vec!["trajectory".to_string(), traj_id.to_string()]);
        }

        let template = EventTemplate {
            kind: 1618, // PULL_REQUEST
            tags,
            content: String::new(),
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        };

        self.publish_event(template).await
    }

    /// Merge a PR (publish kind:1631 status)
    pub async fn merge_pr(&self, pr_id: &str) -> Result<nostr::Event> {
        let template = EventTemplate {
            kind: 1631, // APPLIED/MERGED status
            tags: vec![vec!["e".to_string(), pr_id.to_string()]],
            content: String::new(),
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        };

        self.publish_event(template).await
    }

    /// Claim a bounty (kind 1637)
    pub async fn claim_bounty(
        &self,
        bounty_id: &str,
        pr_id: &str,
        trajectory_session_id: &str,
    ) -> Result<nostr::Event> {
        let template = EventTemplate {
            kind: 1637, // BOUNTY_CLAIM
            tags: vec![
                vec![
                    "e".to_string(),
                    bounty_id.to_string(),
                    "".to_string(),
                    "root".to_string(),
                ],
                vec![
                    "e".to_string(),
                    pr_id.to_string(),
                    "".to_string(),
                    "mention".to_string(),
                ],
                vec!["trajectory".to_string(), trajectory_session_id.to_string()],
            ],
            content: String::new(),
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        };

        self.publish_event(template).await
    }

    /// Release bounty payment via NIP-57 zap (kind 9735)
    pub async fn pay_bounty(
        &self,
        bounty_claim_id: &str,
        recipient_pubkey: &str,
        amount_msats: u64,
    ) -> Result<nostr::Event> {
        let template = EventTemplate {
            kind: 9735, // ZAP_RECEIPT
            tags: vec![
                vec!["e".to_string(), bounty_claim_id.to_string()],
                vec!["p".to_string(), recipient_pubkey.to_string()],
                vec!["amount".to_string(), amount_msats.to_string()],
                vec!["description".to_string(), "Bounty payment".to_string()],
            ],
            content: String::new(),
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        };

        self.publish_event(template).await
    }

    /// Get all events from the relay
    pub async fn get_all_events(&self) -> Vec<nostr::Event> {
        self.relay.get_events().await
    }

    /// Get events by kind
    pub async fn get_events_by_kind(&self, kind: u16) -> Vec<nostr::Event> {
        self.relay.get_events_by_kind(kind).await
    }

    /// Get events by author
    pub async fn get_events_by_author(&self, pubkey: &str) -> Vec<nostr::Event> {
        self.relay.get_events_by_author(pubkey).await
    }

    /// Cleanup (note: relay continues running in background)
    pub async fn shutdown(self) {
        self.relay.shutdown().await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_app_creates_test_identity() {
        let app = TestApp::new().await.unwrap();
        assert_eq!(app.pubkey().len(), 64); // Hex pubkey
        app.shutdown().await;
    }

    #[tokio::test]
    async fn test_app_creates_repository() {
        let app = TestApp::new().await.unwrap();

        let repo = app
            .create_repository("test-repo", "Test Repo", "A test repository")
            .await
            .unwrap();

        assert_eq!(repo.kind, 30617);
        assert_eq!(repo.pubkey, app.pubkey());

        // Verify stored in relay
        let events = app.get_events_by_kind(30617).await;
        assert_eq!(events.len(), 1);

        app.shutdown().await;
    }

    #[tokio::test]
    async fn test_app_creates_issue() {
        let app = TestApp::new().await.unwrap();

        // Create repo first
        let _repo = app
            .create_repository("test-repo", "Test Repo", "Description")
            .await
            .unwrap();

        // Create issue
        let issue = app
            .create_issue("test-repo", "Test Issue", "Issue body")
            .await
            .unwrap();

        assert_eq!(issue.kind, 1621);
        assert_eq!(issue.content, "Issue body");

        // Verify stored
        let issues = app.get_events_by_kind(1621).await;
        assert_eq!(issues.len(), 1);

        app.shutdown().await;
    }

    #[tokio::test]
    async fn test_app_claims_issue() {
        let app = TestApp::new().await.unwrap();

        // Create repo and issue
        let _repo = app
            .create_repository("test-repo", "Test", "Desc")
            .await
            .unwrap();
        let issue = app
            .create_issue("test-repo", "Issue", "Body")
            .await
            .unwrap();

        // Claim issue
        let claim = app.claim_issue(&issue.id).await.unwrap();

        assert_eq!(claim.kind, 1634);
        assert!(claim.tags.iter().any(|t| t.get(1) == Some(&issue.id)));

        app.shutdown().await;
    }

    #[tokio::test]
    async fn test_app_posts_comment() {
        let app = TestApp::new().await.unwrap();

        // Create issue
        let _repo = app
            .create_repository("test-repo", "Test", "Desc")
            .await
            .unwrap();
        let issue = app
            .create_issue("test-repo", "Issue", "Body")
            .await
            .unwrap();

        // Post comment
        let comment = app
            .comment_on_issue(&issue.id, "Great issue!")
            .await
            .unwrap();

        assert_eq!(comment.kind, 1);
        assert_eq!(comment.content, "Great issue!");
        assert!(comment.tags.iter().any(|t| t.get(1) == Some(&issue.id)));

        app.shutdown().await;
    }
}
