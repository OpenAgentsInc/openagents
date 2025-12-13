//! Repository trait for issue storage operations
//!
//! This trait defines the complete interface for issue management,
//! ported from Beads with full feature parity.

use crate::repository::error::Result;
use crate::types::*;

/// Repository trait for issue storage operations
///
/// Implementations can use SQLite, in-memory storage, etc.
/// All methods take &self for thread-safety (interior mutability pattern).
pub trait IssueRepository: Send + Sync {
    // =========================================================================
    // CRUD Operations
    // =========================================================================

    /// Create a new issue
    ///
    /// Generates a random ID with the given prefix.
    fn create(&self, issue: IssueCreate, prefix: &str) -> Result<Issue>;

    /// Create a new issue with a specific ID method
    ///
    /// - `IdMethod::Random`: Generate a random UUID-based ID
    /// - `IdMethod::Hash`: Generate ID from content hash (for deduplication)
    fn create_with_id_method(
        &self,
        issue: IssueCreate,
        method: IdMethod,
        prefix: &str,
    ) -> Result<Issue>;

    /// Get an issue by ID
    ///
    /// Returns NotFound error if issue doesn't exist.
    /// Excludes tombstoned issues unless `get_with_tombstones` is used.
    fn get(&self, id: &str) -> Result<Issue>;

    /// Get an issue by ID, including tombstoned issues
    fn get_with_tombstones(&self, id: &str) -> Result<Issue>;

    /// Check if an issue exists
    fn exists(&self, id: &str) -> Result<bool>;

    /// Update an existing issue
    ///
    /// Records an audit event with the actor (if provided).
    fn update(&self, id: &str, update: IssueUpdate, actor: Option<&str>) -> Result<Issue>;

    /// Soft delete an issue (move to tombstone status)
    ///
    /// The issue will be permanently deleted after the TTL expires.
    fn tombstone(&self, id: &str, reason: Option<&str>, actor: Option<&str>) -> Result<()>;

    /// Permanently delete an issue
    ///
    /// This cannot be undone. Use `tombstone` for soft delete.
    fn purge(&self, id: &str) -> Result<()>;

    /// Restore an issue from tombstone status
    fn restore(&self, id: &str, actor: Option<&str>) -> Result<Issue>;

    // =========================================================================
    // Listing & Rich Filtering
    // =========================================================================

    /// List issues with rich filtering
    ///
    /// Supports AND/OR label filtering, date ranges, pagination, etc.
    fn list(&self, filter: IssueFilter) -> Result<Vec<Issue>>;

    /// Get all issues (convenience method)
    fn all(&self) -> Result<Vec<Issue>> {
        self.list(IssueFilter::default())
    }

    /// Count issues matching filter
    fn count(&self, filter: IssueFilter) -> Result<usize>;

    /// Full-text search issues
    ///
    /// Searches title, description, and notes using FTS5.
    fn search(&self, query: &str, filter: IssueFilter) -> Result<Vec<Issue>>;

    /// Get ready issues (open with no blocking dependencies)
    ///
    /// A task is "ready" if:
    /// - Status is `open`
    /// - Not tombstoned
    /// - No blocking dependencies (blocks, parent-child) on non-closed issues
    fn ready(&self, filter: IssueFilter) -> Result<Vec<Issue>>;

    /// Pick the next ready issue to work on
    ///
    /// Returns the highest priority ready issue.
    fn pick_next(&self, filter: IssueFilter) -> Result<Option<Issue>> {
        let mut f = filter;
        f.limit = Some(1);
        Ok(self.ready(f)?.into_iter().next())
    }

    /// Find stale issues (not updated in N days)
    fn stale(&self, filter: StaleFilter) -> Result<Vec<Issue>>;

    /// Find potential duplicates by content hash
    fn duplicates(&self) -> Result<Vec<DuplicateGroup>>;

    // =========================================================================
    // Status Lifecycle
    // =========================================================================

    /// Start working on an issue (open -> in_progress)
    fn start(&self, id: &str, actor: Option<&str>) -> Result<Issue>;

    /// Close an issue
    fn close(
        &self,
        id: &str,
        reason: Option<&str>,
        commits: Vec<String>,
        actor: Option<&str>,
    ) -> Result<Issue>;

    /// Add a commit SHA to an issue
    fn add_commit(&self, id: &str, sha: &str) -> Result<()>;

    /// Reopen a closed issue
    fn reopen(&self, id: &str, actor: Option<&str>) -> Result<Issue>;

    /// Block an issue
    fn block(&self, id: &str, reason: Option<&str>, actor: Option<&str>) -> Result<Issue>;

    /// Unblock an issue (blocked -> open)
    fn unblock(&self, id: &str, actor: Option<&str>) -> Result<Issue>;

    /// Check if an issue is ready (has no open blocking dependencies)
    fn is_ready(&self, id: &str) -> Result<bool>;

    // =========================================================================
    // Dependencies
    // =========================================================================

    /// Add a dependency relationship
    fn add_dependency(&self, issue_id: &str, dep: Dependency) -> Result<()>;

    /// Remove a dependency relationship
    fn remove_dependency(&self, issue_id: &str, dep_id: &str) -> Result<()>;

    /// Get issues that block this issue
    fn blockers(&self, issue_id: &str) -> Result<Vec<Issue>>;

    /// Get issues blocked by this issue
    fn blocked_by(&self, issue_id: &str) -> Result<Vec<Issue>>;

    /// Get dependency tree for an issue
    fn dependency_tree(&self, issue_id: &str, max_depth: u32) -> Result<DependencyTree>;

    /// Check if adding a dependency would create a cycle
    fn has_cycle(&self, issue_id: &str, dep_id: &str) -> Result<bool>;

    // =========================================================================
    // Labels
    // =========================================================================

    /// Add a label to an issue
    fn add_label(&self, issue_id: &str, label: &str, actor: Option<&str>) -> Result<()>;

    /// Remove a label from an issue
    fn remove_label(&self, issue_id: &str, label: &str, actor: Option<&str>) -> Result<()>;

    /// Get all labels in use with counts
    fn all_labels(&self) -> Result<Vec<LabelCount>>;

    // =========================================================================
    // Comments
    // =========================================================================

    /// Add a comment to an issue
    fn add_comment(&self, issue_id: &str, comment: CommentCreate) -> Result<Comment>;

    /// Get all comments for an issue
    fn comments(&self, issue_id: &str) -> Result<Vec<Comment>>;

    // =========================================================================
    // Events/Audit Trail
    // =========================================================================

    /// Get events for an issue
    fn events(&self, issue_id: &str, limit: Option<usize>) -> Result<Vec<IssueEvent>>;

    /// Get recent events across all issues
    fn recent_events(&self, limit: usize) -> Result<Vec<IssueEvent>>;

    // =========================================================================
    // Compaction
    // =========================================================================

    /// Compact old closed issues
    ///
    /// Reduces storage by archiving details of old closed issues.
    fn compact(&self, older_than_days: u32) -> Result<CompactionResult>;

    // =========================================================================
    // Statistics
    // =========================================================================

    /// Get current statistics
    fn stats(&self) -> Result<IssueStats>;

    /// Get statistics snapshots over time
    fn stats_history(&self, days: u32) -> Result<Vec<StatsSnapshot>>;

    /// Save a statistics snapshot
    fn save_stats_snapshot(&self) -> Result<StatsSnapshot>;

    // =========================================================================
    // Health/Doctor
    // =========================================================================

    /// Run health checks
    fn doctor(&self) -> Result<DoctorReport>;

    /// Repair detected problems
    fn repair(&self, problems: &[DoctorProblem]) -> Result<RepairReport>;

    // =========================================================================
    // Maintenance
    // =========================================================================

    /// Initialize the storage (create tables, etc.)
    fn init(&self) -> Result<()>;

    /// Run pending migrations
    fn migrate(&self) -> Result<MigrationResult>;

    /// Vacuum/optimize storage
    fn vacuum(&self) -> Result<()>;

    /// Clean up expired tombstones
    fn cleanup_tombstones(&self) -> Result<CleanupResult>;
}
