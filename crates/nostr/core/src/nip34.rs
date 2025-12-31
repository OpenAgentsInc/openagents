//! NIP-34: Git Stuff
//!
//! Defines all the ways code collaboration using and adjacent to git can be done using Nostr.
//!
//! Reference: ~/code/nips/34.md

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Event kinds for NIP-34
pub const KIND_REPOSITORY_ANNOUNCEMENT: u16 = 30617;
pub const KIND_REPOSITORY_STATE: u16 = 30618;
pub const KIND_PATCH: u16 = 1617;
pub const KIND_PULL_REQUEST: u16 = 1618;
pub const KIND_PULL_REQUEST_UPDATE: u16 = 1619;
pub const KIND_ISSUE: u16 = 1621;
pub const KIND_STATUS_OPEN: u16 = 1630;
pub const KIND_STATUS_APPLIED: u16 = 1631;
pub const KIND_STATUS_CLOSED: u16 = 1632;
pub const KIND_STATUS_DRAFT: u16 = 1633;
pub const KIND_USER_GRASP_LIST: u16 = 10317;

/// Repository announcement (kind:30617)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepositoryAnnouncement {
    /// Repository identifier (d tag)
    pub repo_id: String,
    /// Human-readable project name
    pub name: String,
    /// Brief description
    pub description: String,
    /// URLs for browsing the repository
    pub web: Vec<String>,
    /// URLs for git cloning
    pub clone: Vec<String>,
    /// Relays to monitor for patches and issues
    pub relays: Vec<String>,
    /// Earliest unique commit ID (for identifying forks)
    pub earliest_unique_commit: Option<String>,
    /// Other recognized maintainers
    pub maintainers: Vec<String>,
    /// Whether this is a personal fork
    pub is_personal_fork: bool,
    /// Hashtags labeling the repository
    pub tags: Vec<String>,
}

impl RepositoryAnnouncement {
    /// Create a new repository announcement
    pub fn new(
        repo_id: impl Into<String>,
        name: impl Into<String>,
        description: impl Into<String>,
    ) -> Self {
        Self {
            repo_id: repo_id.into(),
            name: name.into(),
            description: description.into(),
            web: Vec::new(),
            clone: Vec::new(),
            relays: Vec::new(),
            earliest_unique_commit: None,
            maintainers: Vec::new(),
            is_personal_fork: false,
            tags: Vec::new(),
        }
    }

    /// Add a web browsing URL
    pub fn with_web(mut self, url: impl Into<String>) -> Self {
        self.web.push(url.into());
        self
    }

    /// Add a clone URL
    pub fn with_clone(mut self, url: impl Into<String>) -> Self {
        self.clone.push(url.into());
        self
    }

    /// Add a relay URL
    pub fn with_relay(mut self, url: impl Into<String>) -> Self {
        self.relays.push(url.into());
        self
    }

    /// Set the earliest unique commit (for fork identification)
    pub fn with_earliest_unique_commit(mut self, commit_id: impl Into<String>) -> Self {
        self.earliest_unique_commit = Some(commit_id.into());
        self
    }

    /// Add a maintainer
    pub fn with_maintainer(mut self, pubkey: impl Into<String>) -> Self {
        self.maintainers.push(pubkey.into());
        self
    }

    /// Mark as personal fork
    pub fn as_personal_fork(mut self) -> Self {
        self.is_personal_fork = true;
        self
    }

    /// Add a tag
    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }

    /// Build the event tags
    pub fn build_tags(&self) -> Vec<Vec<String>> {
        let mut tags = vec![
            vec!["d".to_string(), self.repo_id.clone()],
            vec!["name".to_string(), self.name.clone()],
            vec!["description".to_string(), self.description.clone()],
        ];

        for url in &self.web {
            tags.push(vec!["web".to_string(), url.clone()]);
        }

        for url in &self.clone {
            tags.push(vec!["clone".to_string(), url.clone()]);
        }

        for relay in &self.relays {
            tags.push(vec!["relays".to_string(), relay.clone()]);
        }

        if let Some(commit) = &self.earliest_unique_commit {
            tags.push(vec!["r".to_string(), commit.clone(), "euc".to_string()]);
        }

        for maintainer in &self.maintainers {
            tags.push(vec!["maintainers".to_string(), maintainer.clone()]);
        }

        if self.is_personal_fork {
            tags.push(vec!["t".to_string(), "personal-fork".to_string()]);
        }

        for tag in &self.tags {
            tags.push(vec!["t".to_string(), tag.clone()]);
        }

        tags
    }
}

/// Repository state announcement (kind:30618)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepositoryState {
    /// Repository identifier (must match announcement)
    pub repo_id: String,
    /// Branch and tag references
    pub refs: HashMap<String, RefState>,
    /// Current HEAD reference
    pub head: Option<String>,
}

/// State of a git reference (branch or tag)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefState {
    /// Current commit ID
    pub commit_id: String,
    /// Parent commits (for tracking ahead count)
    pub parents: Vec<String>,
}

impl RepositoryState {
    /// Create a new repository state
    pub fn new(repo_id: impl Into<String>) -> Self {
        Self {
            repo_id: repo_id.into(),
            refs: HashMap::new(),
            head: None,
        }
    }

    /// Add a branch reference
    pub fn with_branch(mut self, name: impl Into<String>, commit_id: impl Into<String>) -> Self {
        let key = format!("refs/heads/{}", name.into());
        self.refs.insert(
            key,
            RefState {
                commit_id: commit_id.into(),
                parents: Vec::new(),
            },
        );
        self
    }

    /// Add a tag reference
    pub fn with_tag(mut self, name: impl Into<String>, commit_id: impl Into<String>) -> Self {
        let key = format!("refs/tags/{}", name.into());
        self.refs.insert(
            key,
            RefState {
                commit_id: commit_id.into(),
                parents: Vec::new(),
            },
        );
        self
    }

    /// Set HEAD reference
    pub fn with_head(mut self, branch: impl Into<String>) -> Self {
        self.head = Some(format!("ref: refs/heads/{}", branch.into()));
        self
    }

    /// Build the event tags
    pub fn build_tags(&self) -> Vec<Vec<String>> {
        let mut tags = vec![vec!["d".to_string(), self.repo_id.clone()]];

        for (ref_name, ref_state) in &self.refs {
            let mut tag = vec![ref_name.clone(), ref_state.commit_id.clone()];
            tag.extend(ref_state.parents.clone());
            tags.push(tag);
        }

        if let Some(head) = &self.head {
            tags.push(vec!["HEAD".to_string(), head.clone()]);
        }

        tags
    }
}

/// Patch event (kind:1617)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Patch {
    /// Patch content (git format-patch output)
    pub content: String,
    /// Repository being patched
    pub repository: String,
    /// Repository owner pubkey
    pub repository_owner: String,
    /// Earliest unique commit ID of repo
    pub repo_commit_id: Option<String>,
    /// Additional users to notify
    pub notify: Vec<String>,
    /// Whether this is a root patch
    pub is_root: bool,
    /// Whether this is a root revision
    pub is_root_revision: bool,
    /// Current commit ID (for stable commit ID)
    pub commit_id: Option<String>,
    /// Parent commit ID
    pub parent_commit_id: Option<String>,
    /// PGP signature of commit
    pub commit_pgp_sig: Option<String>,
    /// Committer information
    pub committer: Option<CommitterInfo>,
}

/// Committer information for stable commit IDs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitterInfo {
    pub name: String,
    pub email: String,
    pub timestamp: String,
    pub timezone_offset: String,
}

impl Patch {
    /// Create a new patch
    pub fn new(
        content: impl Into<String>,
        repository: impl Into<String>,
        repository_owner: impl Into<String>,
    ) -> Self {
        Self {
            content: content.into(),
            repository: repository.into(),
            repository_owner: repository_owner.into(),
            repo_commit_id: None,
            notify: Vec::new(),
            is_root: false,
            is_root_revision: false,
            commit_id: None,
            parent_commit_id: None,
            commit_pgp_sig: None,
            committer: None,
        }
    }

    /// Set as root patch
    pub fn as_root(mut self) -> Self {
        self.is_root = true;
        self
    }

    /// Set as root revision
    pub fn as_root_revision(mut self) -> Self {
        self.is_root_revision = true;
        self
    }

    /// Add a user to notify
    pub fn with_notify(mut self, pubkey: impl Into<String>) -> Self {
        self.notify.push(pubkey.into());
        self
    }

    /// Set commit ID for stable commit ID
    pub fn with_commit_id(mut self, commit_id: impl Into<String>) -> Self {
        self.commit_id = Some(commit_id.into());
        self
    }

    /// Set parent commit ID
    pub fn with_parent_commit(mut self, commit_id: impl Into<String>) -> Self {
        self.parent_commit_id = Some(commit_id.into());
        self
    }

    /// Build the event tags
    pub fn build_tags(&self) -> Vec<Vec<String>> {
        let mut tags = vec![
            vec!["a".to_string(), self.repository.clone()],
            vec!["p".to_string(), self.repository_owner.clone()],
        ];

        if let Some(commit_id) = &self.repo_commit_id {
            tags.push(vec!["r".to_string(), commit_id.clone()]);
        }

        for pubkey in &self.notify {
            tags.push(vec!["p".to_string(), pubkey.clone()]);
        }

        if self.is_root {
            tags.push(vec!["t".to_string(), "root".to_string()]);
        }

        if self.is_root_revision {
            tags.push(vec!["t".to_string(), "root-revision".to_string()]);
        }

        if let Some(commit_id) = &self.commit_id {
            tags.push(vec!["commit".to_string(), commit_id.clone()]);
            tags.push(vec!["r".to_string(), commit_id.clone()]);
        }

        if let Some(parent) = &self.parent_commit_id {
            tags.push(vec!["parent-commit".to_string(), parent.clone()]);
        }

        if let Some(sig) = &self.commit_pgp_sig {
            tags.push(vec!["commit-pgp-sig".to_string(), sig.clone()]);
        }

        if let Some(committer) = &self.committer {
            tags.push(vec![
                "committer".to_string(),
                committer.name.clone(),
                committer.email.clone(),
                committer.timestamp.clone(),
                committer.timezone_offset.clone(),
            ]);
        }

        tags
    }
}

/// Pull Request event (kind:1618)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PullRequest {
    /// PR description (markdown)
    pub content: String,
    /// Repository being patched
    pub repository: String,
    /// Repository owner pubkey
    pub repository_owner: String,
    /// Earliest unique commit ID of repo
    pub repo_commit_id: Option<String>,
    /// Additional users to notify
    pub notify: Vec<String>,
    /// PR subject/title
    pub subject: String,
    /// PR labels
    pub labels: Vec<String>,
    /// Current commit ID (tip of PR branch)
    pub commit_id: String,
    /// Clone URLs where commit can be downloaded
    pub clone: Vec<String>,
    /// Recommended branch name
    pub branch_name: Option<String>,
    /// Optional: root patch event ID if this is a revision
    pub replaces_patch: Option<String>,
    /// Optional: most recent common ancestor with target branch
    pub merge_base: Option<String>,
}

impl PullRequest {
    /// Create a new pull request
    pub fn new(
        content: impl Into<String>,
        repository: impl Into<String>,
        repository_owner: impl Into<String>,
        subject: impl Into<String>,
        commit_id: impl Into<String>,
    ) -> Self {
        Self {
            content: content.into(),
            repository: repository.into(),
            repository_owner: repository_owner.into(),
            repo_commit_id: None,
            notify: Vec::new(),
            subject: subject.into(),
            labels: Vec::new(),
            commit_id: commit_id.into(),
            clone: Vec::new(),
            branch_name: None,
            replaces_patch: None,
            merge_base: None,
        }
    }

    /// Add a clone URL
    pub fn with_clone(mut self, url: impl Into<String>) -> Self {
        self.clone.push(url.into());
        self
    }

    /// Add a label
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.labels.push(label.into());
        self
    }

    /// Set branch name
    pub fn with_branch_name(mut self, name: impl Into<String>) -> Self {
        self.branch_name = Some(name.into());
        self
    }

    /// Set merge base
    pub fn with_merge_base(mut self, commit_id: impl Into<String>) -> Self {
        self.merge_base = Some(commit_id.into());
        self
    }

    /// Build the event tags
    pub fn build_tags(&self) -> Vec<Vec<String>> {
        let mut tags = vec![
            vec!["a".to_string(), self.repository.clone()],
            vec!["p".to_string(), self.repository_owner.clone()],
            vec!["subject".to_string(), self.subject.clone()],
            vec!["c".to_string(), self.commit_id.clone()],
        ];

        if let Some(commit_id) = &self.repo_commit_id {
            tags.push(vec!["r".to_string(), commit_id.clone()]);
        }

        for pubkey in &self.notify {
            tags.push(vec!["p".to_string(), pubkey.clone()]);
        }

        for label in &self.labels {
            tags.push(vec!["t".to_string(), label.clone()]);
        }

        for url in &self.clone {
            tags.push(vec!["clone".to_string(), url.clone()]);
        }

        if let Some(branch) = &self.branch_name {
            tags.push(vec!["branch-name".to_string(), branch.clone()]);
        }

        if let Some(patch_id) = &self.replaces_patch {
            tags.push(vec!["e".to_string(), patch_id.clone()]);
        }

        if let Some(merge_base) = &self.merge_base {
            tags.push(vec!["merge-base".to_string(), merge_base.clone()]);
        }

        tags
    }
}

/// Issue event (kind:1621)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Issue {
    /// Issue content (markdown)
    pub content: String,
    /// Repository this issue belongs to
    pub repository: String,
    /// Repository owner pubkey
    pub repository_owner: String,
    /// Issue subject/title
    pub subject: Option<String>,
    /// Issue labels
    pub labels: Vec<String>,
}

impl Issue {
    /// Create a new issue
    pub fn new(
        content: impl Into<String>,
        repository: impl Into<String>,
        repository_owner: impl Into<String>,
    ) -> Self {
        Self {
            content: content.into(),
            repository: repository.into(),
            repository_owner: repository_owner.into(),
            subject: None,
            labels: Vec::new(),
        }
    }

    /// Set issue subject
    pub fn with_subject(mut self, subject: impl Into<String>) -> Self {
        self.subject = Some(subject.into());
        self
    }

    /// Add a label
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.labels.push(label.into());
        self
    }

    /// Build the event tags
    pub fn build_tags(&self) -> Vec<Vec<String>> {
        let mut tags = vec![
            vec!["a".to_string(), self.repository.clone()],
            vec!["p".to_string(), self.repository_owner.clone()],
        ];

        if let Some(subject) = &self.subject {
            tags.push(vec!["subject".to_string(), subject.clone()]);
        }

        for label in &self.labels {
            tags.push(vec!["t".to_string(), label.clone()]);
        }

        tags
    }
}

/// Status types for patches, PRs, and issues
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Status {
    /// Open (kind:1630)
    Open,
    /// Applied/Merged for patches; Resolved for issues (kind:1631)
    Applied,
    /// Closed (kind:1632)
    Closed,
    /// Draft (kind:1633)
    Draft,
}

impl Status {
    /// Get the event kind for this status
    pub fn kind(&self) -> u16 {
        match self {
            Status::Open => KIND_STATUS_OPEN,
            Status::Applied => KIND_STATUS_APPLIED,
            Status::Closed => KIND_STATUS_CLOSED,
            Status::Draft => KIND_STATUS_DRAFT,
        }
    }
}

/// Status update event (kinds:1630-1633)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusUpdate {
    /// Status type
    pub status: Status,
    /// Status comment (markdown)
    pub content: String,
    /// Root issue/PR/patch event ID
    pub root_event_id: String,
    /// Repository owner pubkey
    pub repository_owner: String,
    /// Root event author pubkey
    pub root_author: String,
    /// Optional: accepted revision root ID (for Applied status)
    pub accepted_revision: Option<String>,
    /// Optional: revision author pubkey
    pub revision_author: Option<String>,
    /// Optional: repository reference for subscription efficiency
    pub repository: Option<String>,
    /// Optional: repo commit ID for subscription efficiency
    pub repo_commit_id: Option<String>,
    /// Optional: applied/merged patch event IDs (for Applied status)
    pub applied_patches: Vec<String>,
    /// Optional: merge commit ID (for Applied status)
    pub merge_commit: Option<String>,
    /// Optional: applied commit IDs (for Applied status)
    pub applied_commits: Vec<String>,
}

impl StatusUpdate {
    /// Create a new status update
    pub fn new(
        status: Status,
        content: impl Into<String>,
        root_event_id: impl Into<String>,
        repository_owner: impl Into<String>,
        root_author: impl Into<String>,
    ) -> Self {
        Self {
            status,
            content: content.into(),
            root_event_id: root_event_id.into(),
            repository_owner: repository_owner.into(),
            root_author: root_author.into(),
            accepted_revision: None,
            revision_author: None,
            repository: None,
            repo_commit_id: None,
            applied_patches: Vec::new(),
            merge_commit: None,
            applied_commits: Vec::new(),
        }
    }

    /// Set merge commit (for Applied status)
    pub fn with_merge_commit(mut self, commit_id: impl Into<String>) -> Self {
        self.merge_commit = Some(commit_id.into());
        self
    }

    /// Add applied commit (for Applied status)
    pub fn with_applied_commit(mut self, commit_id: impl Into<String>) -> Self {
        self.applied_commits.push(commit_id.into());
        self
    }

    /// Build the event tags
    pub fn build_tags(&self) -> Vec<Vec<String>> {
        let mut tags = vec![
            vec![
                "e".to_string(),
                self.root_event_id.clone(),
                "".to_string(),
                "root".to_string(),
            ],
            vec!["p".to_string(), self.repository_owner.clone()],
            vec!["p".to_string(), self.root_author.clone()],
        ];

        if let Some(revision_id) = &self.accepted_revision {
            tags.push(vec![
                "e".to_string(),
                revision_id.clone(),
                "".to_string(),
                "reply".to_string(),
            ]);
        }

        if let Some(revision_author) = &self.revision_author {
            tags.push(vec!["p".to_string(), revision_author.clone()]);
        }

        if let Some(repo) = &self.repository {
            tags.push(vec!["a".to_string(), repo.clone()]);
        }

        if let Some(commit_id) = &self.repo_commit_id {
            tags.push(vec!["r".to_string(), commit_id.clone()]);
        }

        for patch_id in &self.applied_patches {
            tags.push(vec!["q".to_string(), patch_id.clone()]);
        }

        if let Some(merge_commit) = &self.merge_commit {
            tags.push(vec!["merge-commit".to_string(), merge_commit.clone()]);
            tags.push(vec!["r".to_string(), merge_commit.clone()]);
        }

        if !self.applied_commits.is_empty() {
            let mut tag = vec!["applied-as-commits".to_string()];
            tag.extend(self.applied_commits.clone());
            tags.push(tag);

            for commit in &self.applied_commits {
                tags.push(vec!["r".to_string(), commit.clone()]);
            }
        }

        tags
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_repository_announcement() {
        let repo = RepositoryAnnouncement::new("my-repo", "My Repo", "A test repository")
            .with_web("https://github.com/user/my-repo")
            .with_clone("https://github.com/user/my-repo.git")
            .with_relay("wss://relay.damus.io")
            .with_earliest_unique_commit("abc123")
            .with_maintainer("npub1maintainer")
            .with_tag("rust");

        let tags = repo.build_tags();
        assert!(tags.iter().any(|t| t[0] == "d" && t[1] == "my-repo"));
        assert!(tags.iter().any(|t| t[0] == "name" && t[1] == "My Repo"));
        assert!(
            tags.iter()
                .any(|t| t[0] == "r" && t.get(2) == Some(&"euc".to_string()))
        );
    }

    #[test]
    fn test_repository_state() {
        let state = RepositoryState::new("my-repo")
            .with_branch("main", "def456")
            .with_tag("v1.0.0", "ghi789")
            .with_head("main");

        let tags = state.build_tags();
        assert!(tags.iter().any(|t| t[0] == "d" && t[1] == "my-repo"));
        assert!(
            tags.iter()
                .any(|t| t[0] == "refs/heads/main" && t[1] == "def456")
        );
        assert!(tags.iter().any(|t| t[0] == "HEAD"));
    }

    #[test]
    fn test_patch() {
        let patch = Patch::new("diff --git a/file.rs", "30617:pubkey:repo-id", "npub1owner")
            .as_root()
            .with_commit_id("commit123");

        let tags = patch.build_tags();
        assert!(tags.iter().any(|t| t[0] == "a"));
        assert!(tags.iter().any(|t| t[0] == "t" && t[1] == "root"));
        assert!(tags.iter().any(|t| t[0] == "commit" && t[1] == "commit123"));
    }

    #[test]
    fn test_pull_request() {
        let pr = PullRequest::new(
            "PR description",
            "30617:pubkey:repo-id",
            "npub1owner",
            "Fix bug",
            "commit456",
        )
        .with_clone("https://github.com/user/repo.git")
        .with_label("bug")
        .with_branch_name("fix-bug");

        let tags = pr.build_tags();
        assert!(tags.iter().any(|t| t[0] == "subject" && t[1] == "Fix bug"));
        assert!(tags.iter().any(|t| t[0] == "c" && t[1] == "commit456"));
        assert!(tags.iter().any(|t| t[0] == "t" && t[1] == "bug"));
    }

    #[test]
    fn test_issue() {
        let issue = Issue::new("Bug report", "30617:pubkey:repo-id", "npub1owner")
            .with_subject("App crashes")
            .with_label("bug")
            .with_label("priority-high");

        let tags = issue.build_tags();
        assert!(
            tags.iter()
                .any(|t| t[0] == "subject" && t[1] == "App crashes")
        );
        assert_eq!(tags.iter().filter(|t| t[0] == "t").count(), 2);
    }

    #[test]
    fn test_status_update() {
        let status = StatusUpdate::new(
            Status::Applied,
            "Merged!",
            "event123",
            "npub1owner",
            "npub1author",
        )
        .with_merge_commit("merge456")
        .with_applied_commit("commit789");

        assert_eq!(status.status.kind(), KIND_STATUS_APPLIED);

        let tags = status.build_tags();
        assert!(tags.iter().any(|t| t[0] == "e" && t[1] == "event123"));
        assert!(
            tags.iter()
                .any(|t| t[0] == "merge-commit" && t[1] == "merge456")
        );
    }

    #[test]
    fn test_status_kinds() {
        assert_eq!(Status::Open.kind(), 1630);
        assert_eq!(Status::Applied.kind(), 1631);
        assert_eq!(Status::Closed.kind(), 1632);
        assert_eq!(Status::Draft.kind(), 1633);
    }
}
