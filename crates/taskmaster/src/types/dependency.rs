//! Dependency relationship types

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

/// Dependency type - categorizes the relationship between issues
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum DependencyType {
    /// Blocks: Issue A blocks Issue B (B waits for A to close)
    /// This affects ready work computation.
    #[default]
    Blocks,
    /// Related: Informational link only (doesn't affect blocking)
    Related,
    /// Parent-Child: Hierarchical relationship (child waits for parent)
    /// This affects ready work computation.
    ParentChild,
    /// Discovered-From: Traceability link (where did this issue originate)
    /// Doesn't affect blocking.
    DiscoveredFrom,
}

impl DependencyType {
    /// Get the string representation
    pub fn as_str(&self) -> &'static str {
        match self {
            DependencyType::Blocks => "blocks",
            DependencyType::Related => "related",
            DependencyType::ParentChild => "parent-child",
            DependencyType::DiscoveredFrom => "discovered-from",
        }
    }

    /// Get human-readable label
    pub fn label(&self) -> &'static str {
        match self {
            DependencyType::Blocks => "Blocks",
            DependencyType::Related => "Related",
            DependencyType::ParentChild => "Parent-Child",
            DependencyType::DiscoveredFrom => "Discovered From",
        }
    }

    /// Check if this dependency type affects issue readiness
    pub fn blocks_readiness(&self) -> bool {
        matches!(self, DependencyType::Blocks | DependencyType::ParentChild)
    }

    /// Get all dependency types
    pub fn all() -> &'static [DependencyType] {
        &[
            DependencyType::Blocks,
            DependencyType::Related,
            DependencyType::ParentChild,
            DependencyType::DiscoveredFrom,
        ]
    }

    /// Get only blocking dependency types
    pub fn blocking_types() -> &'static [DependencyType] {
        &[DependencyType::Blocks, DependencyType::ParentChild]
    }
}

impl fmt::Display for DependencyType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl FromStr for DependencyType {
    type Err = ParseDependencyTypeError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().replace('_', "-").as_str() {
            "blocks" => Ok(DependencyType::Blocks),
            "related" => Ok(DependencyType::Related),
            "parent-child" | "parentchild" => Ok(DependencyType::ParentChild),
            "discovered-from" | "discoveredfrom" => Ok(DependencyType::DiscoveredFrom),
            _ => Err(ParseDependencyTypeError(s.to_string())),
        }
    }
}

/// Error when parsing an invalid dependency type
#[derive(Debug, Clone)]
pub struct ParseDependencyTypeError(pub String);

impl fmt::Display for ParseDependencyTypeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "invalid dependency type '{}', expected one of: blocks, related, parent-child, discovered-from",
            self.0
        )
    }
}

impl std::error::Error for ParseDependencyTypeError {}

/// A dependency relationship between two issues
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Dependency {
    /// ID of the issue that has the dependency
    pub issue_id: String,
    /// ID of the issue being depended upon
    pub depends_on_id: String,
    /// Type of dependency relationship
    #[serde(rename = "type")]
    pub dep_type: DependencyType,
    /// When the dependency was created
    pub created_at: DateTime<Utc>,
}

impl Dependency {
    /// Create a new dependency
    pub fn new(
        issue_id: impl Into<String>,
        depends_on_id: impl Into<String>,
        dep_type: DependencyType,
    ) -> Self {
        Self {
            issue_id: issue_id.into(),
            depends_on_id: depends_on_id.into(),
            dep_type,
            created_at: Utc::now(),
        }
    }

    /// Check if this dependency affects readiness
    pub fn blocks_readiness(&self) -> bool {
        self.dep_type.blocks_readiness()
    }
}

/// Simple dependency reference (for embedding in Issue)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DependencyRef {
    /// ID of the issue being depended upon
    pub id: String,
    /// Type of dependency relationship
    #[serde(rename = "type")]
    pub dep_type: DependencyType,
}

impl DependencyRef {
    /// Create a new dependency reference
    pub fn new(id: impl Into<String>, dep_type: DependencyType) -> Self {
        Self {
            id: id.into(),
            dep_type,
        }
    }

    /// Check if this dependency affects readiness
    pub fn blocks_readiness(&self) -> bool {
        self.dep_type.blocks_readiness()
    }
}

/// Node in a dependency tree (for visualization)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyTreeNode {
    /// Issue ID
    pub id: String,
    /// Issue title
    pub title: String,
    /// Issue status
    pub status: String,
    /// Dependency type from parent
    pub dep_type: Option<DependencyType>,
    /// Depth in tree (0 = root)
    pub depth: u32,
    /// Whether this branch was truncated (max depth reached)
    pub truncated: bool,
    /// Child nodes
    pub children: Vec<DependencyTreeNode>,
}

/// Result of dependency tree traversal
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyTree {
    /// Root node
    pub root: DependencyTreeNode,
    /// Total nodes in tree
    pub total_nodes: usize,
    /// Maximum depth reached
    pub max_depth: u32,
    /// Whether any branches were truncated
    pub has_truncated: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dependency_type_parse() {
        assert_eq!(
            "blocks".parse::<DependencyType>().unwrap(),
            DependencyType::Blocks
        );
        assert_eq!(
            "parent-child".parse::<DependencyType>().unwrap(),
            DependencyType::ParentChild
        );
        assert_eq!(
            "discovered-from".parse::<DependencyType>().unwrap(),
            DependencyType::DiscoveredFrom
        );
        assert!("invalid".parse::<DependencyType>().is_err());
    }

    #[test]
    fn test_blocks_readiness() {
        assert!(DependencyType::Blocks.blocks_readiness());
        assert!(DependencyType::ParentChild.blocks_readiness());
        assert!(!DependencyType::Related.blocks_readiness());
        assert!(!DependencyType::DiscoveredFrom.blocks_readiness());
    }

    #[test]
    fn test_dependency_ref() {
        let dep = DependencyRef::new("issue-123", DependencyType::Blocks);
        assert_eq!(dep.id, "issue-123");
        assert!(dep.blocks_readiness());
    }
}
