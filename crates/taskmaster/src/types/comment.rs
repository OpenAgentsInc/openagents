//! Comment types for issue discussions

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// A comment on an issue
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Comment {
    /// Unique comment ID
    pub id: String,
    /// Issue this comment belongs to
    pub issue_id: String,
    /// Author username/agent name
    pub author: String,
    /// Comment body text
    pub body: String,
    /// When the comment was created
    pub created_at: DateTime<Utc>,
    /// When the comment was last updated (if edited)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<DateTime<Utc>>,
}

impl Comment {
    /// Create a new comment
    pub fn new(
        id: impl Into<String>,
        issue_id: impl Into<String>,
        author: impl Into<String>,
        body: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            issue_id: issue_id.into(),
            author: author.into(),
            body: body.into(),
            created_at: Utc::now(),
            updated_at: None,
        }
    }
}

/// Data for creating a new comment
#[derive(Debug, Clone)]
pub struct CommentCreate {
    /// Author username/agent name
    pub author: String,
    /// Comment body text
    pub body: String,
}

impl CommentCreate {
    /// Create a new CommentCreate
    pub fn new(author: impl Into<String>, body: impl Into<String>) -> Self {
        Self {
            author: author.into(),
            body: body.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_comment_new() {
        let comment = Comment::new("c-1", "issue-1", "alice", "This is a comment");
        assert_eq!(comment.id, "c-1");
        assert_eq!(comment.issue_id, "issue-1");
        assert_eq!(comment.author, "alice");
        assert_eq!(comment.body, "This is a comment");
        assert!(comment.updated_at.is_none());
    }
}
