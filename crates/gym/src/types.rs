//! Shared types for the Gym module

use std::collections::HashSet;

/// Main tab identifiers for the Gym multi-view system
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GymTab {
    /// Current chat + trajectory viewer (reusable)
    Trajectories,
    /// Terminal-Bench Command Center (4 sub-tabs)
    TBCC,
    /// Real-time HillClimber MAP orchestrator visualization
    HillClimber,
    /// TestGen test generation progress + test list
    TestGen,
}

impl Default for GymTab {
    fn default() -> Self {
        Self::Trajectories
    }
}

impl GymTab {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Trajectories => "Trajectories",
            Self::TBCC => "TBCC",
            Self::HillClimber => "HillClimber",
            Self::TestGen => "TestGen",
        }
    }

    pub fn all() -> &'static [GymTab] {
        &[
            GymTab::Trajectories,
            GymTab::TBCC,
            GymTab::HillClimber,
            GymTab::TestGen,
        ]
    }
}

/// Tree node types for the sidebar
#[derive(Debug, Clone, PartialEq)]
pub enum TreeNode {
    /// Expandable category (folder)
    Category {
        id: String,
        label: String,
        icon: &'static str,
        children: Vec<TreeNode>,
    },
    /// Individual item (session, run, suite)
    Item {
        id: String,
        kind: TreeItemKind,
        label: String,
        metadata: String, // e.g., "23 steps", "87% pass"
        status: ItemStatus,
    },
}

/// Kind of tree item
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TreeItemKind {
    Session,
    Trajectory,
    HillClimberRun,
    TestGenSuite,
}

impl TreeItemKind {
    /// Get the corresponding GymTab for this item type
    pub fn tab(&self) -> GymTab {
        match self {
            TreeItemKind::Session | TreeItemKind::Trajectory => GymTab::Trajectories,
            TreeItemKind::HillClimberRun => GymTab::HillClimber,
            TreeItemKind::TestGenSuite => GymTab::TestGen,
        }
    }
}

/// Status of a tree item
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ItemStatus {
    Idle,
    Running { progress: f32 },
    Success,
    Failed,
    Partial { score: f32 },
}

/// Sidebar state management
#[derive(Debug, Clone, Default)]
pub struct SidebarState {
    /// Set of expanded category IDs
    pub expanded: HashSet<String>,
    /// Currently selected item ID
    pub selected: Option<String>,
}

impl SidebarState {
    pub fn new() -> Self {
        Self {
            expanded: HashSet::new(),
            selected: None,
        }
    }

    pub fn toggle_expand(&mut self, id: &str) {
        if self.expanded.contains(id) {
            self.expanded.remove(id);
        } else {
            self.expanded.insert(id.to_string());
        }
    }

    pub fn is_expanded(&self, id: &str) -> bool {
        self.expanded.contains(id)
    }

    pub fn select(&mut self, id: String) {
        self.selected = Some(id);
    }

    pub fn is_selected(&self, id: &str) -> bool {
        self.selected.as_deref() == Some(id)
    }
}
