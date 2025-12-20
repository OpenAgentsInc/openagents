//! Core types for the marketplace

use serde::{Deserialize, Serialize};

/// Type of marketplace item
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MarketplaceItemType {
    /// A plugin (extends functionality via MCP)
    Plugin,
    /// A skill (predefined task template)
    Skill,
    /// An agent (autonomous worker)
    Agent,
}

impl MarketplaceItemType {
    /// Get the item type as a string
    pub fn as_str(&self) -> &'static str {
        match self {
            MarketplaceItemType::Plugin => "plugin",
            MarketplaceItemType::Skill => "skill",
            MarketplaceItemType::Agent => "agent",
        }
    }
}

/// Status of a marketplace item
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ItemStatus {
    /// Available for installation
    Available,
    /// Currently being installed
    Installing,
    /// Successfully installed
    Installed,
    /// Installation failed
    Failed,
    /// Update available
    UpdateAvailable,
}

impl ItemStatus {
    /// Get the status as a string
    pub fn as_str(&self) -> &'static str {
        match self {
            ItemStatus::Available => "available",
            ItemStatus::Installing => "installing",
            ItemStatus::Installed => "installed",
            ItemStatus::Failed => "failed",
            ItemStatus::UpdateAvailable => "update_available",
        }
    }

    /// Check if the item is installed
    pub fn is_installed(&self) -> bool {
        matches!(self, ItemStatus::Installed | ItemStatus::UpdateAvailable)
    }

    /// Check if the item is in a transitional state
    pub fn is_transitional(&self) -> bool {
        matches!(self, ItemStatus::Installing)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_item_type_as_str() {
        assert_eq!(MarketplaceItemType::Plugin.as_str(), "plugin");
        assert_eq!(MarketplaceItemType::Skill.as_str(), "skill");
        assert_eq!(MarketplaceItemType::Agent.as_str(), "agent");
    }

    #[test]
    fn test_item_status_as_str() {
        assert_eq!(ItemStatus::Available.as_str(), "available");
        assert_eq!(ItemStatus::Installing.as_str(), "installing");
        assert_eq!(ItemStatus::Installed.as_str(), "installed");
        assert_eq!(ItemStatus::Failed.as_str(), "failed");
        assert_eq!(ItemStatus::UpdateAvailable.as_str(), "update_available");
    }

    #[test]
    fn test_item_status_is_installed() {
        assert!(!ItemStatus::Available.is_installed());
        assert!(!ItemStatus::Installing.is_installed());
        assert!(ItemStatus::Installed.is_installed());
        assert!(!ItemStatus::Failed.is_installed());
        assert!(ItemStatus::UpdateAvailable.is_installed());
    }

    #[test]
    fn test_item_status_is_transitional() {
        assert!(!ItemStatus::Available.is_transitional());
        assert!(ItemStatus::Installing.is_transitional());
        assert!(!ItemStatus::Installed.is_transitional());
        assert!(!ItemStatus::Failed.is_transitional());
        assert!(!ItemStatus::UpdateAvailable.is_transitional());
    }

    #[test]
    fn test_serde_roundtrip() {
        let item_type = MarketplaceItemType::Skill;
        let json = serde_json::to_string(&item_type).unwrap();
        assert_eq!(json, "\"skill\"");
        let deserialized: MarketplaceItemType = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, item_type);

        let status = ItemStatus::Installed;
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, "\"installed\"");
        let deserialized: ItemStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, status);
    }
}
