//! Rich filtering types for issue queries

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::issue_type::IssueType;
use super::priority::Priority;
use super::status::IssueStatus;

/// Sort policy for query results
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SortPolicy {
    /// Priority first, then oldest (default)
    /// Recent issues (< 48h) sorted by priority, older by age
    #[default]
    Hybrid,
    /// Pure priority, ties use newest
    Priority,
    /// FIFO - oldest first (for backlog clearing)
    Oldest,
    /// Reverse FIFO - newest first
    Newest,
    /// Recently updated first
    RecentlyUpdated,
}

impl SortPolicy {
    /// Get the string representation
    pub fn as_str(&self) -> &'static str {
        match self {
            SortPolicy::Hybrid => "hybrid",
            SortPolicy::Priority => "priority",
            SortPolicy::Oldest => "oldest",
            SortPolicy::Newest => "newest",
            SortPolicy::RecentlyUpdated => "recently_updated",
        }
    }
}

/// Assignee filter options
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssigneeFilter {
    /// Must be assigned to this user
    Is(String),
    /// Must NOT be assigned to this user
    IsNot(String),
    /// Must have no assignee
    Unassigned,
    /// Must have some assignee
    Assigned,
}

/// Label filter with AND/OR logic
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LabelFilter {
    /// All labels must match (AND semantics)
    All(Vec<String>),
    /// Any label must match (OR semantics)
    Any(Vec<String>),
    /// Must have no labels
    None,
    /// Complex expression
    Expr(LabelExpr),
}

impl LabelFilter {
    /// Create an AND filter
    pub fn all(labels: impl IntoIterator<Item = impl Into<String>>) -> Self {
        LabelFilter::All(labels.into_iter().map(|s| s.into()).collect())
    }

    /// Create an OR filter
    pub fn any(labels: impl IntoIterator<Item = impl Into<String>>) -> Self {
        LabelFilter::Any(labels.into_iter().map(|s| s.into()).collect())
    }
}

/// Complex label expression for advanced filtering
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LabelExpr {
    /// Must have this label
    Has(String),
    /// Must NOT have this label
    Not(Box<LabelExpr>),
    /// All expressions must match
    And(Vec<LabelExpr>),
    /// At least one expression must match
    Or(Vec<LabelExpr>),
}

impl LabelExpr {
    /// Create a Has expression
    pub fn has(label: impl Into<String>) -> Self {
        LabelExpr::Has(label.into())
    }

    /// Create a Not expression
    pub fn not(expr: LabelExpr) -> Self {
        LabelExpr::Not(Box::new(expr))
    }

    /// Create an And expression
    pub fn and(exprs: Vec<LabelExpr>) -> Self {
        LabelExpr::And(exprs)
    }

    /// Create an Or expression
    pub fn or(exprs: Vec<LabelExpr>) -> Self {
        LabelExpr::Or(exprs)
    }
}

/// Rich filter for issue queries
/// Supports AND/OR label filtering, date ranges, and more
#[derive(Debug, Clone, Default)]
pub struct IssueFilter {
    /// Filter by status(es)
    pub status: Option<Vec<IssueStatus>>,

    /// Filter by priority(ies)
    pub priority: Option<Vec<Priority>>,

    /// Filter by type(s)
    pub issue_type: Option<Vec<IssueType>>,

    /// Filter by assignee
    pub assignee: Option<AssigneeFilter>,

    /// Label filter with AND/OR logic
    pub labels: Option<LabelFilter>,

    /// Title contains (case-insensitive)
    pub title_contains: Option<String>,

    /// Description contains (case-insensitive)
    pub description_contains: Option<String>,

    /// Created after this date
    pub created_after: Option<DateTime<Utc>>,

    /// Created before this date
    pub created_before: Option<DateTime<Utc>>,

    /// Updated after this date
    pub updated_after: Option<DateTime<Utc>>,

    /// Updated before this date
    pub updated_before: Option<DateTime<Utc>>,

    /// Closed after this date
    pub closed_after: Option<DateTime<Utc>>,

    /// Closed before this date
    pub closed_before: Option<DateTime<Utc>>,

    /// Only issues with empty description
    pub empty_description: bool,

    /// Only issues with no labels
    pub no_labels: bool,

    /// Include tombstoned (soft-deleted) issues
    pub include_tombstones: bool,

    /// Sort order
    pub sort: SortPolicy,

    /// Maximum results to return
    pub limit: Option<usize>,

    /// Offset for pagination
    pub offset: Option<usize>,
}

impl IssueFilter {
    /// Create a new empty filter
    pub fn new() -> Self {
        Self::default()
    }

    /// Filter by single status
    pub fn status(mut self, status: IssueStatus) -> Self {
        self.status = Some(vec![status]);
        self
    }

    /// Filter by multiple statuses
    pub fn statuses(mut self, statuses: Vec<IssueStatus>) -> Self {
        self.status = Some(statuses);
        self
    }

    /// Filter by single priority
    pub fn priority(mut self, priority: Priority) -> Self {
        self.priority = Some(vec![priority]);
        self
    }

    /// Filter by multiple priorities
    pub fn priorities(mut self, priorities: Vec<Priority>) -> Self {
        self.priority = Some(priorities);
        self
    }

    /// Filter by single type
    pub fn issue_type(mut self, issue_type: IssueType) -> Self {
        self.issue_type = Some(vec![issue_type]);
        self
    }

    /// Filter by assignee
    pub fn assignee(mut self, filter: AssigneeFilter) -> Self {
        self.assignee = Some(filter);
        self
    }

    /// Filter by assigned to specific user
    pub fn assigned_to(mut self, user: impl Into<String>) -> Self {
        self.assignee = Some(AssigneeFilter::Is(user.into()));
        self
    }

    /// Filter for unassigned issues
    pub fn unassigned(mut self) -> Self {
        self.assignee = Some(AssigneeFilter::Unassigned);
        self
    }

    /// Filter by labels (AND semantics)
    pub fn labels_all(mut self, labels: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.labels = Some(LabelFilter::all(labels));
        self
    }

    /// Filter by labels (OR semantics)
    pub fn labels_any(mut self, labels: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.labels = Some(LabelFilter::any(labels));
        self
    }

    /// Filter by title containing text
    pub fn title_contains(mut self, text: impl Into<String>) -> Self {
        self.title_contains = Some(text.into());
        self
    }

    /// Filter by created after date
    pub fn created_after(mut self, date: DateTime<Utc>) -> Self {
        self.created_after = Some(date);
        self
    }

    /// Filter by created before date
    pub fn created_before(mut self, date: DateTime<Utc>) -> Self {
        self.created_before = Some(date);
        self
    }

    /// Include tombstoned issues
    pub fn include_tombstones(mut self) -> Self {
        self.include_tombstones = true;
        self
    }

    /// Set sort policy
    pub fn sort(mut self, policy: SortPolicy) -> Self {
        self.sort = policy;
        self
    }

    /// Set result limit
    pub fn limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }

    /// Set pagination offset
    pub fn offset(mut self, offset: usize) -> Self {
        self.offset = Some(offset);
        self
    }

    /// Create a filter for open issues only
    pub fn open() -> Self {
        Self::new().status(IssueStatus::Open)
    }

    /// Create a filter for ready issues (open, sorted for work)
    pub fn ready() -> Self {
        Self::new()
            .status(IssueStatus::Open)
            .sort(SortPolicy::Hybrid)
    }
}

/// Filter for stale issues (not updated in N days)
#[derive(Debug, Clone)]
pub struct StaleFilter {
    /// Days since last update
    pub days: u32,
    /// Status to filter (default: all non-closed)
    pub status: Option<IssueStatus>,
    /// Maximum results
    pub limit: Option<usize>,
}

impl StaleFilter {
    /// Create a new stale filter
    pub fn new(days: u32) -> Self {
        Self {
            days,
            status: None,
            limit: None,
        }
    }

    /// Filter by status
    pub fn status(mut self, status: IssueStatus) -> Self {
        self.status = Some(status);
        self
    }

    /// Set limit
    pub fn limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }
}

/// Result of duplicate detection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DuplicateGroup {
    /// Content hash shared by duplicates
    pub content_hash: String,
    /// Issue IDs that share this hash
    pub issue_ids: Vec<String>,
    /// Representative title
    pub title: String,
}

/// Label with usage count
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LabelCount {
    /// Label name
    pub label: String,
    /// Number of issues with this label
    pub count: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filter_builder() {
        let filter = IssueFilter::new()
            .status(IssueStatus::Open)
            .priority(Priority::High)
            .assigned_to("alice")
            .labels_all(["urgent", "backend"])
            .limit(10);

        assert_eq!(filter.status, Some(vec![IssueStatus::Open]));
        assert_eq!(filter.priority, Some(vec![Priority::High]));
        assert!(matches!(filter.assignee, Some(AssigneeFilter::Is(ref s)) if s == "alice"));
        assert!(matches!(filter.labels, Some(LabelFilter::All(_))));
        assert_eq!(filter.limit, Some(10));
    }

    #[test]
    fn test_label_filter() {
        let all = LabelFilter::all(["a", "b", "c"]);
        assert!(matches!(all, LabelFilter::All(ref v) if v.len() == 3));

        let any = LabelFilter::any(["x", "y"]);
        assert!(matches!(any, LabelFilter::Any(ref v) if v.len() == 2));
    }

    #[test]
    fn test_label_expr() {
        // (has "urgent" AND has "backend") OR NOT has "wontfix"
        let expr = LabelExpr::or(vec![
            LabelExpr::and(vec![LabelExpr::has("urgent"), LabelExpr::has("backend")]),
            LabelExpr::not(LabelExpr::has("wontfix")),
        ]);

        assert!(matches!(expr, LabelExpr::Or(_)));
    }
}
