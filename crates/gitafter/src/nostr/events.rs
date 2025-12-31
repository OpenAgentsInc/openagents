//! NIP-34 event builders for GitAfter
//!
//! Provides builder pattern for creating Nostr events related to git collaboration.
//! All builders return [`EventTemplate`] which must be signed before publishing.
//!
//! ## Standard NIP-34 Builders
//!
//! - [`PullRequestBuilder`] - kind:1618, create pull requests with commits
//! - [`PatchBuilder`] - kind:1617, create patches from git diffs
//! - [`StatusEventBuilder`] - kinds:1630-1633, set PR/patch status
//!
//! ## GitAfter Extensions
//!
//! These extend NIP-34 with agent-native workflows:
//!
//! - [`IssueClaimBuilder`] - kind:1634, agent claims issue for work
//! - [`BountyOfferBuilder`] - kind:1636, attach Lightning bounty to issue
//! - [`WorkAssignmentBuilder`] - kind:1635, maintainer assigns work to agent
//! - [`BountyClaimBuilder`] - kind:1637, claim bounty payment on PR merge
//!
//! ## Usage
//!
//! All builders follow the same pattern:
//!
//! ```rust
//! use gitafter::nostr::events::PullRequestBuilder;
//!
//! // Build event template
//! let template = PullRequestBuilder::new(
//!     "30617:pubkey:repo-id",       // Repository address
//!     "Fix authentication bug",     // Subject/title
//!     "This PR fixes the bug by...", // Description
//! )
//! .commit("abc123def456")            // Required: commit ID
//! .clone_url("https://github.com/user/repo.git") // Required: clone URL
//! .trajectory("session_xyz")         // Optional: trajectory session ID
//! .build();
//!
//! // Sign with identity (requires wallet integration)
//! // let event = identity.sign_event(template)?;
//! // client.publish_event(event).await?;
//! ```
//!
//! ## Stacked Diffs
//!
//! Pull requests support stacked diffs for breaking large changes into reviewable layers:
//!
//! ```rust
//! use gitafter::nostr::events::PullRequestBuilder;
//!
//! // Layer 1: Foundation
//! let layer1 = PullRequestBuilder::new(
//!     "30617:pubkey:repo-id",
//!     "Layer 1: Add FooService interface",
//!     "Defines the interface for FooService",
//! )
//! .commit("abc123")
//! .clone_url("https://github.com/user/repo.git")
//! .stack("stack_uuid_123")  // Groups all layers
//! .layer(1, 4)              // Layer 1 of 4
//! .build();
//!
//! // Layer 2: Build on Layer 1
//! let layer2 = PullRequestBuilder::new(
//!     "30617:pubkey:repo-id",
//!     "Layer 2: Implement FooService",
//!     "Concrete implementation of FooService",
//! )
//! .commit("def456")
//! .clone_url("https://github.com/user/repo.git")
//! .depends_on("layer1_event_id")  // Must merge after Layer 1
//! .stack("stack_uuid_123")
//! .layer(2, 4)
//! .build();
//! ```
//!
//! Tags used for stacked diffs:
//! - `depends_on`: Event ID of dependency PR (must be merged first)
//! - `stack`: UUID grouping related PRs
//! - `layer`: Position in stack (e.g., "2 of 4")
//!
//! ## Trajectory Integration
//!
//! PRs can link to agent trajectory sessions for transparent work verification:
//!
//! ```rust,ignore
//! use gitafter::nostr::events::PullRequestBuilder;
//!
//! let pr = PullRequestBuilder::new(...)
//!     .trajectory("session_id")          // NIP-SA trajectory session
//!     .trajectory_hash("sha256_hash")    // Hash for verification
//!     .build();
//! ```
//!
//! ## Event Publishing
//!
//! Event templates must be signed before publishing:
//!
//! ```rust,ignore
//! // 1. Build template
//! let template = PullRequestBuilder::new(...).build();
//!
//! // 2. Sign with identity (requires wallet::UnifiedIdentity)
//! let event = identity.sign_event(template)?;
//!
//! // 3. Publish to relays (not yet implemented in GitAfter)
//! client.publish_event(event).await?;
//!
//! // 4. Cache locally for immediate display
//! client.cache_event(event).await?;
//! ```
//!
//! ## Testing
//!
//! All builders have comprehensive unit tests:
//!
//! ```bash
//! cargo test -p gitafter events
//! ```

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
    /// * `repo_address` - The repository address tag (e.g., "30617:\<pubkey\>:\<repo-id\>")
    /// * `issue_author_pubkey` - The pubkey of the issue author
    ///
    /// # Examples
    ///
    /// ```
    /// use gitafter::nostr::events::IssueClaimBuilder;
    ///
    /// let claim = IssueClaimBuilder::new(
    ///     "abc123...",  // issue event ID
    ///     "30617:npub1...:openagents",  // repo address
    ///     "npub1...",  // issue author
    /// )
    /// .content("I'll implement this feature")
    /// .estimate(3600)  // 1 hour estimate in seconds
    /// .trajectory("session-uuid");
    ///
    /// let event_template = claim.build();
    /// ```
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
            vec![
                "e".to_string(),
                self.issue_event_id,
                "".to_string(),
                "root".to_string(),
            ],
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
/// A bounty offer event attaches a bitcoin bounty to an issue or PR layer.
/// Supports both single-issue bounties and per-layer bounties for stacked PRs.
#[allow(dead_code)]
pub struct BountyOfferBuilder {
    issue_event_id: String,
    repo_address: String,
    amount_sats: u64,
    expiry_timestamp: Option<u64>,
    conditions: Vec<String>,
    /// Stack UUID for per-layer bounties
    stack_id: Option<String>,
    /// Layer info for per-layer bounties: (current_layer, total_layers)
    layer: Option<(u32, u32)>,
}

#[allow(dead_code)]
impl BountyOfferBuilder {
    /// Create a new bounty offer builder
    ///
    /// # Arguments
    /// * `issue_event_id` - The event ID of the issue
    /// * `repo_address` - The repository address tag (e.g., "30617:\<pubkey\>:\<repo-id\>")
    /// * `amount_sats` - The bounty amount in satoshis
    ///
    /// # Examples
    ///
    /// ```
    /// use gitafter::nostr::events::BountyOfferBuilder;
    ///
    /// let bounty = BountyOfferBuilder::new(
    ///     "issue-event-id",
    ///     "30617:npub1...:openagents",
    ///     50000,  // 50k sats
    /// )
    /// .expiry(1735689600)  // Unix timestamp
    /// .condition("Must include tests")
    /// .condition("Must pass CI");
    ///
    /// let event_template = bounty.build();
    /// ```
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
            stack_id: None,
            layer: None,
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

    /// Set stack ID for per-layer bounties in stacked PRs
    ///
    /// # Example
    /// ```
    /// # use gitafter::nostr::events::BountyOfferBuilder;
    /// let bounty = BountyOfferBuilder::new("pr-event-id", "30617:pubkey:repo", 25000)
    ///     .stack("550e8400-e29b-41d4-a716-446655440000")
    ///     .layer(2, 4); // Layer 2 of 4
    /// ```
    pub fn stack(mut self, stack_id: impl Into<String>) -> Self {
        self.stack_id = Some(stack_id.into());
        self
    }

    /// Set layer information for per-layer bounties
    ///
    /// # Arguments
    /// * `current` - Current layer number (1-indexed)
    /// * `total` - Total number of layers in the stack
    pub fn layer(mut self, current: u32, total: u32) -> Self {
        self.layer = Some((current, total));
        self
    }

    /// Build the event template
    pub fn build(self) -> EventTemplate {
        let mut tags = vec![
            // Reference to issue (root marker)
            vec![
                "e".to_string(),
                self.issue_event_id,
                "".to_string(),
                "root".to_string(),
            ],
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

        // Add stack ID for per-layer bounties
        if let Some(stack_id) = self.stack_id {
            tags.push(vec!["stack".to_string(), stack_id]);
        }

        // Add layer info for per-layer bounties
        if let Some((current, total)) = self.layer {
            tags.push(vec![
                "layer".to_string(),
                current.to_string(),
                total.to_string(),
            ]);
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
///
/// # Examples
///
/// ```
/// use gitafter::nostr::events::WorkAssignmentBuilder;
///
/// let assignment = WorkAssignmentBuilder::new(
///     "issue-event-id-123",
///     "30617:npub1...:openagents",
///     "npub1agent...",
/// )
/// .content("Assigned to @agent_alice - this aligns with your expertise")
/// .build();
///
/// assert_eq!(assignment.kind, 1635);
/// ```
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
    /// * `repo_address` - The repository address tag (e.g., "30617:\<pubkey\>:\<repo-id\>")
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
            vec![
                "e".to_string(),
                self.issue_event_id,
                "".to_string(),
                "root".to_string(),
            ],
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
///
/// # Examples
///
/// ```
/// use gitafter::nostr::events::BountyClaimBuilder;
///
/// let claim = BountyClaimBuilder::new(
///     "bounty-offer-event-id",
///     "merged-pr-event-id",
///     "30617:npub1...:openagents",
///     "trajectory-session-abc123",
///     "sha256-hash-of-trajectory-events",
/// )
/// .lightning_address("agent@getalby.com")
/// .relay("wss://relay.nostr.bg")
/// .build();
///
/// assert_eq!(claim.kind, 1637);
/// ```
#[allow(dead_code)]
pub struct BountyClaimBuilder {
    bounty_event_id: String,
    merged_pr_event_id: String,
    repo_address: String,
    trajectory_session_id: String,
    trajectory_hash: String,
    lightning_address: Option<String>,
    invoice: Option<String>,
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
            invoice: None,
            relay_hint: None,
        }
    }

    /// Set the Lightning address (lud16) for payment
    pub fn lightning_address(mut self, address: impl Into<String>) -> Self {
        self.lightning_address = Some(address.into());
        self
    }

    /// Set a BOLT11 invoice for direct payout
    pub fn invoice(mut self, invoice: impl Into<String>) -> Self {
        self.invoice = Some(invoice.into());
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
            vec![
                "e".to_string(),
                self.bounty_event_id,
                "".to_string(),
                "root".to_string(),
            ],
            // Reference to merged PR
            vec![
                "e".to_string(),
                self.merged_pr_event_id,
                "".to_string(),
                "mention".to_string(),
            ],
            // Repository reference
            vec!["a".to_string(), self.repo_address],
        ];

        // Add trajectory with optional relay hint
        if let Some(relay) = &self.relay_hint {
            tags.push(vec![
                "trajectory".to_string(),
                self.trajectory_session_id.clone(),
                relay.clone(),
            ]);
        } else {
            tags.push(vec![
                "trajectory".to_string(),
                self.trajectory_session_id.clone(),
            ]);
        }

        // Add trajectory hash
        tags.push(vec!["trajectory_hash".to_string(), self.trajectory_hash]);

        // Add optional lightning address
        if let Some(lud16) = self.lightning_address {
            tags.push(vec!["lud16".to_string(), lud16]);
        }

        if let Some(invoice) = self.invoice {
            tags.push(vec!["invoice".to_string(), invoice]);
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

/// Builder for creating pull request events (kind:1618)
///
/// Pull request events represent code contributions with optional trajectory
/// tracking and stacked diffs support.
///
/// # Examples
///
/// ## Basic Pull Request
///
/// ```
/// use gitafter::nostr::events::PullRequestBuilder;
///
/// let pr = PullRequestBuilder::new(
///     "30617:npub1...:openagents",
///     "Fix authentication bug",
///     "This PR fixes the auth timeout issue by...",
/// )
/// .commit("abc123def456789")
/// .clone_url("https://github.com/user/repo.git")
/// .build();
///
/// assert_eq!(pr.kind, 1618);
/// ```
///
/// ## With Trajectory Tracking
///
/// ```
/// use gitafter::nostr::events::PullRequestBuilder;
///
/// let pr = PullRequestBuilder::new(
///     "30617:npub1...:openagents",
///     "Add payment integration",
///     "Integrates Breez SDK for Lightning payments",
/// )
/// .commit("def456")
/// .clone_url("https://github.com/user/repo.git")
/// .trajectory("session-uuid-123")
/// .trajectory_hash("sha256-hash-abc")
/// .build();
/// ```
///
/// ## Stacked Diffs
///
/// ```
/// use gitafter::nostr::events::PullRequestBuilder;
///
/// // Layer 2 depends on Layer 1
/// let layer2 = PullRequestBuilder::new(
///     "30617:npub1...:openagents",
///     "Layer 2: Wire service into handlers",
///     "Connects FooService to HTTP handlers",
/// )
/// .commit("ghi789")
/// .clone_url("https://github.com/user/repo.git")
/// .depends_on("layer1-event-id")
/// .stack("stack-uuid-456")
/// .layer(2, 4)
/// .build();
/// ```
#[allow(dead_code)]
pub struct PullRequestBuilder {
    repo_address: String,
    subject: String,
    content: String,
    commit_id: Option<String>,
    clone_url: Option<String>,
    trajectory_session_id: Option<String>,
    trajectory_hash: Option<String>,
    // Stacked diffs support
    depends_on: Option<String>,
    stack_id: Option<String>,
    layer_position: Option<(u32, u32)>, // (current, total)
}

#[allow(dead_code)]
impl PullRequestBuilder {
    /// Create a new pull request builder
    ///
    /// # Arguments
    /// * `repo_address` - The repository address tag (e.g., "30617:\<pubkey\>:\<repo-id\>")
    /// * `subject` - The PR title/subject
    /// * `content` - The PR description
    pub fn new(
        repo_address: impl Into<String>,
        subject: impl Into<String>,
        content: impl Into<String>,
    ) -> Self {
        Self {
            repo_address: repo_address.into(),
            subject: subject.into(),
            content: content.into(),
            commit_id: None,
            clone_url: None,
            trajectory_session_id: None,
            trajectory_hash: None,
            depends_on: None,
            stack_id: None,
            layer_position: None,
        }
    }

    /// Set the commit ID for this PR
    pub fn commit(mut self, commit_id: impl Into<String>) -> Self {
        self.commit_id = Some(commit_id.into());
        self
    }

    /// Set the clone URL for fetching this PR
    pub fn clone_url(mut self, url: impl Into<String>) -> Self {
        self.clone_url = Some(url.into());
        self
    }

    /// Set the trajectory session ID that tracks the work
    pub fn trajectory(mut self, session_id: impl Into<String>) -> Self {
        self.trajectory_session_id = Some(session_id.into());
        self
    }

    /// Set the trajectory hash for verification
    pub fn trajectory_hash(mut self, hash: impl Into<String>) -> Self {
        self.trajectory_hash = Some(hash.into());
        self
    }

    /// Set the dependency on another PR (for stacked diffs)
    ///
    /// # Arguments
    /// * `pr_event_id` - The event ID of the PR this one depends on
    pub fn depends_on(mut self, pr_event_id: impl Into<String>) -> Self {
        self.depends_on = Some(pr_event_id.into());
        self
    }

    /// Set the stack ID to group related PRs (for stacked diffs)
    ///
    /// # Arguments
    /// * `stack_id` - A UUID or identifier grouping PRs in this stack
    pub fn stack(mut self, stack_id: impl Into<String>) -> Self {
        self.stack_id = Some(stack_id.into());
        self
    }

    /// Set the layer position in the stack (for stacked diffs)
    ///
    /// # Arguments
    /// * `current` - The current layer number (1-indexed)
    /// * `total` - The total number of layers in the stack
    pub fn layer(mut self, current: u32, total: u32) -> Self {
        self.layer_position = Some((current, total));
        self
    }

    /// Build the event template
    ///
    /// # Panics
    /// Panics if required fields are missing:
    /// - `commit_id` must be set via `.commit()`
    /// - `clone_url` must be set via `.clone_url()`
    pub fn build(self) -> EventTemplate {
        // Validate required fields for NIP-34 compliance
        let commit = self
            .commit_id
            .expect("commit_id is required for NIP-34 pull requests - use .commit()");
        let url = self
            .clone_url
            .expect("clone_url is required for NIP-34 pull requests - use .clone_url()");

        let mut tags = vec![
            // Repository reference
            vec!["a".to_string(), self.repo_address],
            // Subject/title
            vec!["subject".to_string(), self.subject],
            // Commit ID (required)
            vec!["c".to_string(), commit],
            // Clone URL (required)
            vec!["clone".to_string(), url],
        ];

        // Add optional trajectory
        if let Some(session_id) = self.trajectory_session_id {
            tags.push(vec!["trajectory".to_string(), session_id]);
        }

        // Add optional trajectory hash
        if let Some(hash) = self.trajectory_hash {
            tags.push(vec!["trajectory_hash".to_string(), hash]);
        }

        // Stacked diffs tags
        if let Some(dep_id) = self.depends_on {
            tags.push(vec!["depends_on".to_string(), dep_id]);
        }

        if let Some(stack) = self.stack_id {
            tags.push(vec!["stack".to_string(), stack]);
        }

        if let Some((current, total)) = self.layer_position {
            tags.push(vec![
                "layer".to_string(),
                current.to_string(),
                total.to_string(),
            ]);
        }

        EventTemplate {
            created_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            kind: 1618, // Pull Request
            tags,
            content: self.content,
        }
    }
}

/// Builder for creating patch events (kind:1617)
///
/// A patch event contains a git diff/patch for a repository.
///
/// # Examples
///
/// ```
/// use gitafter::nostr::events::PatchBuilder;
///
/// let patch_content = r#"diff --git a/src/main.rs b/src/main.rs
/// index abc123..def456 100644
/// --- a/src/main.rs
/// +++ b/src/main.rs
/// @@ -1,3 +1,4 @@
/// +// Fixed typo
///  fn main() {
///      println!("Hello");
///  }"#;
///
/// let patch = PatchBuilder::new(
///     "30617:npub1...:openagents",
///     "Fix typo in main.rs",
///     patch_content,
/// )
/// .description("Corrects spelling mistake in comment")
/// .build();
///
/// assert_eq!(patch.kind, 1617);
/// ```
#[allow(dead_code)]
pub struct PatchBuilder {
    repo_address: String,
    subject: String,
    patch_content: String,
    description: Option<String>,
}

#[allow(dead_code)]
impl PatchBuilder {
    /// Create a new patch builder
    ///
    /// # Arguments
    /// * `repo_address` - The repository address tag (e.g., "30617:\<pubkey\>:\<repo-id\>")
    /// * `subject` - The patch title/subject
    /// * `patch_content` - The git diff/patch content
    pub fn new(
        repo_address: impl Into<String>,
        subject: impl Into<String>,
        patch_content: impl Into<String>,
    ) -> Self {
        Self {
            repo_address: repo_address.into(),
            subject: subject.into(),
            patch_content: patch_content.into(),
            description: None,
        }
    }

    /// Set an optional description for the patch
    pub fn description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }

    /// Build the event template
    pub fn build(self) -> EventTemplate {
        let tags = vec![
            // Repository reference
            vec!["a".to_string(), self.repo_address],
            // Subject/title
            vec!["subject".to_string(), self.subject],
        ];

        // Patch content goes in the event content
        // Description is separate if provided
        let content = if let Some(desc) = self.description {
            format!("{}\n\n{}", desc, self.patch_content)
        } else {
            self.patch_content
        };

        EventTemplate {
            created_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            kind: 1617, // Patch
            tags,
            content,
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
///
/// # Examples
///
/// ## Mark PR as Merged
///
/// ```
/// use gitafter::nostr::events::StatusEventBuilder;
///
/// let status = StatusEventBuilder::applied(
///     "pr-event-id-123",
///     "30617:npub1...:openagents",
/// )
/// .reason("All checks passed, trajectory verified")
/// .build();
///
/// assert_eq!(status.kind, 1631);
/// ```
///
/// ## Mark PR as Closed
///
/// ```
/// use gitafter::nostr::events::StatusEventBuilder;
///
/// let status = StatusEventBuilder::closed(
///     "pr-event-id-456",
///     "30617:npub1...:openagents",
/// )
/// .reason("Superseded by #789")
/// .build();
///
/// assert_eq!(status.kind, 1632);
/// ```
///
/// ## Mark as Draft
///
/// ```
/// use gitafter::nostr::events::StatusEventBuilder;
///
/// let status = StatusEventBuilder::draft(
///     "pr-event-id-789",
///     "30617:npub1...:openagents",
/// )
/// .build();
///
/// assert_eq!(status.kind, 1633);
/// ```
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
    /// * `repo_address` - The repository address tag (e.g., "30617:\<pubkey\>:\<repo-id\>")
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
            vec![
                "e".to_string(),
                self.target_event_id,
                "".to_string(),
                "root".to_string(),
            ],
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
        let template = IssueClaimBuilder::new("issue123", "30617:pubkey123:repo456", "author789")
            .content("I'll work on this. Estimated completion: 2 hours.")
            .trajectory("session_abc")
            .estimate(7200)
            .build();

        assert_eq!(template.kind, 1634);
        assert!(
            template
                .tags
                .iter()
                .any(|t| t[0] == "e" && t[1] == "issue123")
        );
        assert!(
            template
                .tags
                .iter()
                .any(|t| t[0] == "a" && t[1] == "30617:pubkey123:repo456")
        );
        assert!(
            template
                .tags
                .iter()
                .any(|t| t[0] == "p" && t[1] == "author789")
        );
        assert!(
            template
                .tags
                .iter()
                .any(|t| t[0] == "trajectory" && t[1] == "session_abc")
        );
        assert!(
            template
                .tags
                .iter()
                .any(|t| t[0] == "estimate" && t[1] == "7200")
        );
    }

    #[test]
    fn test_bounty_offer_builder() {
        let template = BountyOfferBuilder::new("issue123", "30617:pubkey123:repo456", 50000)
            .expiry(1700000000)
            .condition("must include tests")
            .condition("must pass CI")
            .build();

        assert_eq!(template.kind, 1636);
        assert!(
            template
                .tags
                .iter()
                .any(|t| t[0] == "e" && t[1] == "issue123")
        );
        assert!(
            template
                .tags
                .iter()
                .any(|t| t[0] == "amount" && t[1] == "50000")
        );
        assert!(
            template
                .tags
                .iter()
                .any(|t| t[0] == "expiry" && t[1] == "1700000000")
        );
        assert!(
            template
                .tags
                .iter()
                .filter(|t| t[0] == "conditions")
                .count()
                == 2
        );
    }

    #[test]
    fn test_work_assignment_builder() {
        let template =
            WorkAssignmentBuilder::new("issue123", "30617:pubkey123:repo456", "agent789")
                .content("Assigned to @agent")
                .build();

        assert_eq!(template.kind, 1635);
        assert!(template.tags.iter().any(|t| t[0] == "p"
            && t[1] == "agent789"
            && t.get(3) == Some(&"assignee".to_string())));
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
        assert!(
            template
                .tags
                .iter()
                .any(|t| t[0] == "trajectory_hash" && t[1] == "hash_def")
        );
        assert!(
            template
                .tags
                .iter()
                .any(|t| t[0] == "lud16" && t[1] == "agent@getalby.com")
        );
    }

    #[test]
    fn test_pull_request_builder() {
        let template = PullRequestBuilder::new(
            "30617:pubkey123:repo456",
            "Fix authentication bug",
            "This PR fixes the auth bug by...",
        )
        .commit("abc123def456")
        .clone_url("https://github.com/user/repo.git")
        .trajectory("session_xyz")
        .trajectory_hash("hash_abc")
        .build();

        assert_eq!(template.kind, 1618);
        assert!(
            template
                .tags
                .iter()
                .any(|t| t[0] == "a" && t[1] == "30617:pubkey123:repo456")
        );
        assert!(
            template
                .tags
                .iter()
                .any(|t| t[0] == "subject" && t[1] == "Fix authentication bug")
        );
        assert!(
            template
                .tags
                .iter()
                .any(|t| t[0] == "c" && t[1] == "abc123def456")
        );
        assert!(
            template
                .tags
                .iter()
                .any(|t| t[0] == "clone" && t[1] == "https://github.com/user/repo.git")
        );
        assert!(
            template
                .tags
                .iter()
                .any(|t| t[0] == "trajectory" && t[1] == "session_xyz")
        );
        assert!(
            template
                .tags
                .iter()
                .any(|t| t[0] == "trajectory_hash" && t[1] == "hash_abc")
        );
    }

    #[test]
    fn test_pull_request_builder_stacked() {
        let template = PullRequestBuilder::new(
            "30617:pubkey123:repo456",
            "Layer 2: Wire service into auth flow",
            "This layer wires the FooService...",
        )
        .clone_url("https://github.com/test/repo.git")
        .commit("def456ghi789")
        .depends_on("pr_layer_1_event_id")
        .stack("stack_uuid_123")
        .layer(2, 4)
        .build();

        assert_eq!(template.kind, 1618);
        assert!(
            template
                .tags
                .iter()
                .any(|t| t[0] == "depends_on" && t[1] == "pr_layer_1_event_id")
        );
        assert!(
            template
                .tags
                .iter()
                .any(|t| t[0] == "stack" && t[1] == "stack_uuid_123")
        );
        assert!(
            template
                .tags
                .iter()
                .any(|t| t.len() == 3 && t[0] == "layer" && t[1] == "2" && t[2] == "4")
        );
    }

    #[test]
    fn test_patch_builder() {
        let patch_content = "diff --git a/file.rs b/file.rs\nindex abc123..def456 100644\n--- a/file.rs\n+++ b/file.rs\n@@ -1,3 +1,4 @@\n+// New comment\n fn main() {";

        let template = PatchBuilder::new(
            "30617:pubkey123:repo456",
            "Fix typo in documentation",
            patch_content,
        )
        .description("This patch fixes a typo in the docs")
        .build();

        assert_eq!(template.kind, 1617);
        assert!(
            template
                .tags
                .iter()
                .any(|t| t[0] == "a" && t[1] == "30617:pubkey123:repo456")
        );
        assert!(
            template
                .tags
                .iter()
                .any(|t| t[0] == "subject" && t[1] == "Fix typo in documentation")
        );
        assert!(
            template
                .content
                .contains("This patch fixes a typo in the docs")
        );
        assert!(template.content.contains("diff --git"));
    }
}

/// Builder for NIP-34 kind:30617 repository announcement events
///
/// Creates repository announcement events that advertise a git repository on Nostr.
/// These are parameterized replaceable events keyed by the repository identifier.
///
/// # Example
///
/// ```rust
/// use gitafter::nostr::events::RepositoryAnnouncementBuilder;
///
/// let template = RepositoryAnnouncementBuilder::new(
///     "my-awesome-project",
///     "My Awesome Project",
/// )
/// .description("A revolutionary new protocol for...")
/// .clone_url("git@github.com:user/my-awesome-project.git")
/// .clone_url("https://github.com/user/my-awesome-project.git")
/// .web_url("https://github.com/user/my-awesome-project")
/// .maintainer("npub1abc...")
/// .earliest_commit("abc123def456")
/// .default_branch("main")
/// .build();
///
/// // Sign and publish
/// // let event = identity.sign_event(template)?;
/// // client.publish_event(event).await?;
/// ```
///
/// # Tags
///
/// - `d`: Repository identifier (required, makes it parameterized replaceable)
/// - `name`: Human-readable repository name
/// - `description`: Repository description
/// - `clone`: Clone URL (can have multiple for different protocols)
/// - `web`: Web interface URL
/// - `p`: Maintainer pubkeys (can have multiple)
/// - `r`: Earliest unique commit for fork tracking
/// - `default_branch`: Default branch name
/// - `language`: Primary language tag
/// - `topic`: Repository topics (can have multiple)
pub struct RepositoryAnnouncementBuilder {
    identifier: String,
    name: String,
    description: Option<String>,
    clone_urls: Vec<String>,
    web_url: Option<String>,
    maintainers: Vec<String>,
    earliest_commit: Option<String>,
    default_branch: Option<String>,
    language: Option<String>,
    topics: Vec<String>,
}

#[allow(dead_code)]
impl RepositoryAnnouncementBuilder {
    /// Create a new repository announcement builder
    ///
    /// # Arguments
    /// * `identifier` - Unique repository identifier (e.g., "my-awesome-project")
    /// * `name` - Human-readable repository name
    ///
    /// # Examples
    ///
    /// ```
    /// use gitafter::nostr::events::RepositoryAnnouncementBuilder;
    ///
    /// let builder = RepositoryAnnouncementBuilder::new("openagents", "OpenAgents")
    ///     .description("Decentralized autonomous agents on Nostr")
    ///     .clone_url("https://github.com/OpenAgentsInc/openagents.git")
    ///     .web_url("https://github.com/OpenAgentsInc/openagents")
    ///     .maintainer("npub1...");
    ///
    /// let event_template = builder.build();
    /// ```
    pub fn new(identifier: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            identifier: identifier.into(),
            name: name.into(),
            description: None,
            clone_urls: Vec::new(),
            web_url: None,
            maintainers: Vec::new(),
            earliest_commit: None,
            default_branch: None,
            language: None,
            topics: Vec::new(),
        }
    }

    /// Set the repository description
    pub fn description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }

    /// Add a clone URL (can be called multiple times for different protocols)
    ///
    /// # Arguments
    /// * `url` - Clone URL (e.g., "git@github.com:user/repo.git" or "https://...")
    pub fn clone_url(mut self, url: impl Into<String>) -> Self {
        self.clone_urls.push(url.into());
        self
    }

    /// Set the web interface URL
    pub fn web_url(mut self, url: impl Into<String>) -> Self {
        self.web_url = Some(url.into());
        self
    }

    /// Add a maintainer pubkey (can be called multiple times)
    ///
    /// # Arguments
    /// * `pubkey` - Maintainer Nostr pubkey (npub or hex)
    pub fn maintainer(mut self, pubkey: impl Into<String>) -> Self {
        self.maintainers.push(pubkey.into());
        self
    }

    /// Set the earliest unique commit SHA for fork tracking
    ///
    /// # Arguments
    /// * `sha` - Git commit SHA of the earliest commit unique to this repository
    pub fn earliest_commit(mut self, sha: impl Into<String>) -> Self {
        self.earliest_commit = Some(sha.into());
        self
    }

    /// Set the default branch name
    pub fn default_branch(mut self, branch: impl Into<String>) -> Self {
        self.default_branch = Some(branch.into());
        self
    }

    /// Set the primary repository language
    pub fn language(mut self, language: impl Into<String>) -> Self {
        self.language = Some(language.into());
        self
    }

    /// Add a topic tag (can be called multiple times)
    pub fn topic(mut self, topic: impl Into<String>) -> Self {
        self.topics.push(topic.into());
        self
    }

    /// Build the repository announcement event template
    ///
    /// Returns an [`EventTemplate`] with kind:30617 that must be signed before publishing.
    pub fn build(self) -> EventTemplate {
        let mut tags = vec![
            vec!["d".to_string(), self.identifier],
            vec!["name".to_string(), self.name.clone()],
        ];

        if let Some(desc) = &self.description {
            tags.push(vec!["description".to_string(), desc.clone()]);
        }

        for url in &self.clone_urls {
            tags.push(vec!["clone".to_string(), url.clone()]);
        }

        if let Some(web) = &self.web_url {
            tags.push(vec!["web".to_string(), web.clone()]);
        }

        for maintainer in &self.maintainers {
            tags.push(vec!["p".to_string(), maintainer.clone()]);
        }

        if let Some(commit) = &self.earliest_commit {
            tags.push(vec!["r".to_string(), commit.clone()]);
        }

        if let Some(branch) = &self.default_branch {
            tags.push(vec!["default_branch".to_string(), branch.clone()]);
        }

        if let Some(language) = &self.language {
            tags.push(vec!["language".to_string(), language.clone()]);
        }

        for topic in &self.topics {
            tags.push(vec!["topic".to_string(), topic.clone()]);
        }

        EventTemplate {
            created_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            kind: 30617,
            content: self.description.unwrap_or_default(),
            tags,
        }
    }
}

#[cfg(test)]
mod repository_announcement_tests {
    use super::*;

    #[test]
    fn test_minimal_repository_announcement() {
        let template = RepositoryAnnouncementBuilder::new("test-repo", "Test Repository").build();

        assert_eq!(template.kind, 30617);
        assert_eq!(template.content, "");
        assert!(
            template
                .tags
                .contains(&vec!["d".to_string(), "test-repo".to_string()])
        );
        assert!(
            template
                .tags
                .contains(&vec!["name".to_string(), "Test Repository".to_string()])
        );
    }

    #[test]
    fn test_full_repository_announcement() {
        let template = RepositoryAnnouncementBuilder::new("awesome-project", "Awesome Project")
            .description("A revolutionary protocol")
            .clone_url("git@github.com:user/awesome-project.git")
            .clone_url("https://github.com/user/awesome-project.git")
            .web_url("https://github.com/user/awesome-project")
            .maintainer("npub1alice")
            .maintainer("npub1bob")
            .earliest_commit("abc123def456")
            .default_branch("main")
            .language("rust")
            .topic("nostr")
            .topic("git")
            .build();

        assert_eq!(template.kind, 30617);
        assert_eq!(template.content, "A revolutionary protocol");

        // Check required tags
        assert!(
            template
                .tags
                .contains(&vec!["d".to_string(), "awesome-project".to_string()])
        );
        assert!(
            template
                .tags
                .contains(&vec!["name".to_string(), "Awesome Project".to_string()])
        );

        // Check optional tags
        assert!(template.tags.contains(&vec![
            "description".to_string(),
            "A revolutionary protocol".to_string()
        ]));
        assert!(template.tags.contains(&vec![
            "clone".to_string(),
            "git@github.com:user/awesome-project.git".to_string()
        ]));
        assert!(template.tags.contains(&vec![
            "clone".to_string(),
            "https://github.com/user/awesome-project.git".to_string()
        ]));
        assert!(template.tags.contains(&vec![
            "web".to_string(),
            "https://github.com/user/awesome-project".to_string()
        ]));
        assert!(
            template
                .tags
                .contains(&vec!["p".to_string(), "npub1alice".to_string()])
        );
        assert!(
            template
                .tags
                .contains(&vec!["p".to_string(), "npub1bob".to_string()])
        );
        assert!(
            template
                .tags
                .contains(&vec!["r".to_string(), "abc123def456".to_string()])
        );
        assert!(
            template
                .tags
                .contains(&vec!["default_branch".to_string(), "main".to_string()])
        );
        assert!(
            template
                .tags
                .contains(&vec!["language".to_string(), "rust".to_string()])
        );
        assert!(
            template
                .tags
                .contains(&vec!["topic".to_string(), "nostr".to_string()])
        );
        assert!(
            template
                .tags
                .contains(&vec!["topic".to_string(), "git".to_string()])
        );
    }

    #[test]
    fn test_multiple_clone_urls() {
        let template = RepositoryAnnouncementBuilder::new("multi-clone", "Multi Clone")
            .clone_url("git@github.com:user/repo.git")
            .clone_url("https://github.com/user/repo.git")
            .clone_url("git@gitlab.com:user/repo.git")
            .build();

        let clone_tags: Vec<_> = template
            .tags
            .iter()
            .filter(|tag| tag.len() >= 2 && tag[0] == "clone")
            .collect();

        assert_eq!(clone_tags.len(), 3);
    }

    #[test]
    fn test_multiple_maintainers() {
        let template = RepositoryAnnouncementBuilder::new("team-repo", "Team Repository")
            .maintainer("npub1alice")
            .maintainer("npub1bob")
            .maintainer("npub1charlie")
            .build();

        let maintainer_tags: Vec<_> = template
            .tags
            .iter()
            .filter(|tag| tag.len() >= 2 && tag[0] == "p")
            .collect();

        assert_eq!(maintainer_tags.len(), 3);
    }
}

/// Builder for NIP-57 zap request events (kind:9734)
///
/// Zap requests are sent to LNURL callbacks to request Lightning payment receipts.
/// The request must be signed before sending to the callback endpoint.
///
/// # Examples
///
/// ## Basic Zap
///
/// ```
/// use gitafter::nostr::events::ZapRequestBuilder;
///
/// let zap = ZapRequestBuilder::new("recipient-pubkey-hex")
///     .amount_sats(21)
///     .relay("wss://relay.damus.io")
///     .build();
///
/// assert_eq!(zap.kind, 9734);
/// ```
///
/// ## Zap with Message
///
/// ```
/// use gitafter::nostr::events::ZapRequestBuilder;
///
/// let zap = ZapRequestBuilder::new("recipient-pubkey-hex")
///     .amount_sats(1000)
///     .content("Great work on this PR!")
///     .relay("wss://relay.nostr.bg")
///     .relay("wss://nos.lol")
///     .build();
/// ```
///
/// ## Zap a Specific Event
///
/// ```
/// use gitafter::nostr::events::ZapRequestBuilder;
///
/// let zap = ZapRequestBuilder::new("recipient-pubkey-hex")
///     .amount_sats(5000)
///     .event("pr-event-id-to-zap")
///     .content("Excellent implementation!")
///     .relay("wss://relay.damus.io")
///     .build();
/// ```
pub struct ZapRequestBuilder {
    recipient_pubkey: String,
    zapped_event: Option<String>,
    relays: Vec<String>,
    amount_msats: Option<u64>,
    content: String,
}

impl ZapRequestBuilder {
    /// Create a new zap request builder
    ///
    /// # Arguments
    /// * `recipient_pubkey` - Hex-encoded public key of the recipient
    pub fn new(recipient_pubkey: impl Into<String>) -> Self {
        Self {
            recipient_pubkey: recipient_pubkey.into(),
            zapped_event: None,
            relays: Vec::new(),
            amount_msats: None,
            content: String::new(),
        }
    }

    /// Set the amount to zap in millisatoshis
    ///
    /// # Arguments
    /// * `msats` - Amount in millisatoshis (1 sat = 1000 msats)
    pub fn amount_msats(mut self, msats: u64) -> Self {
        self.amount_msats = Some(msats);
        self
    }

    /// Set the amount to zap in satoshis (convenience method)
    ///
    /// # Arguments
    /// * `sats` - Amount in satoshis
    pub fn amount_sats(self, sats: u64) -> Self {
        self.amount_msats(sats.saturating_mul(1000))
    }

    /// Add a relay where the zap receipt should be published
    ///
    /// # Arguments
    /// * `relay` - WebSocket URL of the relay
    pub fn relay(mut self, relay: impl Into<String>) -> Self {
        self.relays.push(relay.into());
        self
    }

    /// Set the event being zapped (optional)
    ///
    /// # Arguments
    /// * `event_id` - Hex-encoded event ID
    pub fn event(mut self, event_id: impl Into<String>) -> Self {
        self.zapped_event = Some(event_id.into());
        self
    }

    /// Set an optional message/comment for the zap
    ///
    /// # Arguments
    /// * `content` - Message text
    pub fn content(mut self, content: impl Into<String>) -> Self {
        self.content = content.into();
        self
    }

    /// Build the event template
    ///
    /// The template must be signed before being sent to the LNURL callback.
    pub fn build(self) -> nostr::EventTemplate {
        let mut tags = vec![vec!["p".to_string(), self.recipient_pubkey.clone()]];

        // Add optional event tag
        if let Some(event_id) = self.zapped_event {
            tags.push(vec!["e".to_string(), event_id]);
        }

        // Add relays tag
        if !self.relays.is_empty() {
            let mut relays_tag = vec!["relays".to_string()];
            relays_tag.extend(self.relays);
            tags.push(relays_tag);
        }

        // Add amount tag if specified
        if let Some(amount) = self.amount_msats {
            tags.push(vec!["amount".to_string(), amount.to_string()]);
        }

        nostr::EventTemplate {
            kind: 9734,
            content: self.content,
            tags,
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        }
    }
}

#[cfg(test)]
mod zap_tests {
    use super::*;

    #[test]
    fn test_minimal_zap_request() {
        let template = ZapRequestBuilder::new("recipient_pubkey").build();

        assert_eq!(template.kind, 9734);
        assert!(
            template
                .tags
                .contains(&vec!["p".to_string(), "recipient_pubkey".to_string()])
        );
    }

    #[test]
    fn test_full_zap_request() {
        let template = ZapRequestBuilder::new("recipient_pubkey")
            .amount_sats(21)
            .relay("wss://relay.damus.io")
            .relay("wss://relay.snort.social")
            .event("event123")
            .content("Great work!")
            .build();

        assert_eq!(template.kind, 9734);
        assert_eq!(template.content, "Great work!");
        assert!(
            template
                .tags
                .contains(&vec!["p".to_string(), "recipient_pubkey".to_string()])
        );
        assert!(
            template
                .tags
                .contains(&vec!["e".to_string(), "event123".to_string()])
        );
        assert!(
            template
                .tags
                .contains(&vec!["amount".to_string(), "21000".to_string()])
        );

        // Check relays tag
        let relays_tag: Vec<_> = template
            .tags
            .iter()
            .filter(|tag| tag.len() >= 1 && tag[0] == "relays")
            .collect();
        assert_eq!(relays_tag.len(), 1);
        assert_eq!(relays_tag[0].len(), 3); // "relays" + 2 relay URLs
    }

    #[test]
    fn test_zap_amount_conversion() {
        let template = ZapRequestBuilder::new("recipient").amount_sats(1).build();

        let amount_tag = template
            .tags
            .iter()
            .find(|tag| tag.len() >= 2 && tag[0] == "amount")
            .expect("amount tag should exist");

        assert_eq!(amount_tag[1], "1000"); // 1 sat = 1000 msats
    }
}
