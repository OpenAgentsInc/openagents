//! Skill browsing and search via NIP-89 discovery
//!
//! This module implements skill discovery using the Nostr network via NIP-89.

use nostr::HandlerInfo;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Errors that can occur during skill browsing operations
#[derive(Debug, Error)]
pub enum BrowseError {
    #[error("network error: {0}")]
    Network(String),

    #[error("parse error: {0}")]
    Parse(String),

    #[error("no skills found matching criteria")]
    NoSkillsFound,
}

/// Sort order for skill listings
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortBy {
    /// Most recently published first
    Recent,
    /// Alphabetical by name
    Name,
    /// Most popular (by recommendation count)
    Popular,
    /// Lowest price first
    Price,
}

/// Category filter for skills
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SkillCategory {
    /// Development tools
    DevTools,
    /// Data processing
    DataProcessing,
    /// Communication
    Communication,
    /// Web scraping
    WebScraping,
    /// Code generation
    CodeGeneration,
    /// Testing
    Testing,
    /// Other category
    Other(String),
}

impl SkillCategory {
    pub fn as_str(&self) -> &str {
        match self {
            SkillCategory::DevTools => "dev-tools",
            SkillCategory::DataProcessing => "data-processing",
            SkillCategory::Communication => "communication",
            SkillCategory::WebScraping => "web-scraping",
            SkillCategory::CodeGeneration => "code-generation",
            SkillCategory::Testing => "testing",
            SkillCategory::Other(s) => s.as_str(),
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "dev-tools" => SkillCategory::DevTools,
            "data-processing" => SkillCategory::DataProcessing,
            "communication" => SkillCategory::Communication,
            "web-scraping" => SkillCategory::WebScraping,
            "code-generation" => SkillCategory::CodeGeneration,
            "testing" => SkillCategory::Testing,
            other => SkillCategory::Other(other.to_string()),
        }
    }
}

/// Search filters for browsing skills
#[derive(Debug, Clone, Default)]
pub struct SearchFilters {
    /// Filter by category
    pub category: Option<SkillCategory>,
    /// Filter by capability (e.g., "fetch", "parse")
    pub capability: Option<String>,
    /// Maximum price in sats (None = any price)
    pub max_price_sats: Option<u64>,
    /// Only show free skills
    pub free_only: bool,
    /// Text search query
    pub query: Option<String>,
}

impl SearchFilters {
    /// Create empty filters
    pub fn new() -> Self {
        Self::default()
    }

    /// Filter by category
    pub fn with_category(mut self, category: SkillCategory) -> Self {
        self.category = Some(category);
        self
    }

    /// Filter by capability
    pub fn with_capability(mut self, capability: impl Into<String>) -> Self {
        self.capability = Some(capability.into());
        self
    }

    /// Filter by maximum price
    pub fn with_max_price(mut self, max_price_sats: u64) -> Self {
        self.max_price_sats = Some(max_price_sats);
        self
    }

    /// Only show free skills
    pub fn free_only(mut self) -> Self {
        self.free_only = true;
        self
    }

    /// Add text search query
    pub fn with_query(mut self, query: impl Into<String>) -> Self {
        self.query = Some(query.into());
        self
    }
}

/// Skill listing from marketplace discovery
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillListing {
    /// Skill ID (handler pubkey)
    pub id: String,
    /// Skill name
    pub name: String,
    /// Short description
    pub description: String,
    /// Version
    pub version: String,
    /// Creator pubkey
    pub creator_pubkey: String,
    /// Capabilities offered
    pub capabilities: Vec<String>,
    /// Price in sats (None = free)
    pub price_sats: Option<u64>,
    /// Pricing model (e.g., "per-call", "per-token")
    pub price_model: Option<String>,
    /// Icon URL
    pub icon_url: Option<String>,
    /// Website URL
    pub website: Option<String>,
    /// Recommendation count
    pub recommendation_count: u64,
}

impl SkillListing {
    /// Convert from NIP-89 HandlerInfo
    pub fn from_handler_info(handler: HandlerInfo) -> Self {
        let (price_sats, price_model) = handler
            .pricing
            .as_ref()
            .map(|p| (Some(p.amount), p.model.clone()))
            .unwrap_or((None, None));

        Self {
            id: handler.pubkey.clone(),
            name: handler.metadata.name.clone(),
            description: handler.metadata.description.clone(),
            version: "1.0.0".to_string(), // TODO: extract from metadata
            creator_pubkey: handler.pubkey,
            capabilities: handler.capabilities,
            price_sats,
            price_model,
            icon_url: handler.metadata.icon_url,
            website: handler.metadata.website,
            recommendation_count: 0, // Will be populated by discovery
        }
    }

    /// Check if skill matches the given filters
    pub fn matches_filters(&self, filters: &SearchFilters) -> bool {
        // Check free-only filter
        if filters.free_only && self.price_sats.is_some() {
            return false;
        }

        // Check max price filter
        if let Some(max_price) = filters.max_price_sats {
            if let Some(price) = self.price_sats {
                if price > max_price {
                    return false;
                }
            }
        }

        // Check capability filter
        if let Some(capability) = &filters.capability {
            if !self.capabilities.iter().any(|c| c == capability) {
                return false;
            }
        }

        // Check text query (search in name and description)
        if let Some(query) = &filters.query {
            let query_lower = query.to_lowercase();
            let matches = self.name.to_lowercase().contains(&query_lower)
                || self.description.to_lowercase().contains(&query_lower);
            if !matches {
                return false;
            }
        }

        true
    }
}

/// Skill browser for NIP-89 discovery
pub struct SkillBrowser {
    // Future: add relay pool connection
}

impl SkillBrowser {
    /// Create a new skill browser
    pub fn new() -> Self {
        Self {}
    }

    /// Browse all skills
    ///
    /// This will fetch all skill handlers from relays and return them as listings.
    pub async fn browse(
        &self,
        filters: SearchFilters,
        sort_by: SortBy,
    ) -> Result<Vec<SkillListing>, BrowseError> {
        // TODO: Fetch from relays
        // For now, return empty list
        let mut listings = Vec::new();

        // Filter and sort
        listings.retain(|listing: &SkillListing| listing.matches_filters(&filters));
        self.sort_listings(&mut listings, sort_by);

        Ok(listings)
    }

    /// Search for skills by query
    pub async fn search(
        &self,
        query: impl Into<String>,
        sort_by: SortBy,
    ) -> Result<Vec<SkillListing>, BrowseError> {
        let filters = SearchFilters::new().with_query(query);
        self.browse(filters, sort_by).await
    }

    /// Get a specific skill by ID
    pub async fn get_skill(&self, _skill_id: &str) -> Result<SkillListing, BrowseError> {
        // TODO: Fetch specific skill from relays
        Err(BrowseError::NoSkillsFound)
    }

    /// Sort listings according to the specified order
    fn sort_listings(&self, listings: &mut [SkillListing], sort_by: SortBy) {
        match sort_by {
            SortBy::Recent => {
                // TODO: Sort by publication timestamp when available
            }
            SortBy::Name => {
                listings.sort_by(|a, b| a.name.cmp(&b.name));
            }
            SortBy::Popular => {
                listings.sort_by(|a, b| b.recommendation_count.cmp(&a.recommendation_count));
            }
            SortBy::Price => {
                listings.sort_by(|a, b| {
                    match (a.price_sats, b.price_sats) {
                        (None, None) => std::cmp::Ordering::Equal,
                        (None, Some(_)) => std::cmp::Ordering::Less, // Free comes first
                        (Some(_), None) => std::cmp::Ordering::Greater,
                        (Some(a_price), Some(b_price)) => a_price.cmp(&b_price),
                    }
                });
            }
        }
    }
}

impl Default for SkillBrowser {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_skill_category_conversions() {
        assert_eq!(SkillCategory::DevTools.as_str(), "dev-tools");
        assert_eq!(SkillCategory::from_str("dev-tools"), SkillCategory::DevTools);

        let custom = SkillCategory::Other("custom".to_string());
        assert_eq!(custom.as_str(), "custom");
        assert_eq!(SkillCategory::from_str("custom"), custom);
    }

    #[test]
    fn test_search_filters_builder() {
        let filters = SearchFilters::new()
            .with_category(SkillCategory::DevTools)
            .with_capability("fetch")
            .with_max_price(1000)
            .with_query("test");

        assert_eq!(filters.category, Some(SkillCategory::DevTools));
        assert_eq!(filters.capability, Some("fetch".to_string()));
        assert_eq!(filters.max_price_sats, Some(1000));
        assert_eq!(filters.query, Some("test".to_string()));
    }

    #[test]
    fn test_skill_listing_matches_filters_free_only() {
        let skill = SkillListing {
            id: "test".to_string(),
            name: "Test".to_string(),
            description: "Test skill".to_string(),
            version: "1.0.0".to_string(),
            creator_pubkey: "creator".to_string(),
            capabilities: vec![],
            price_sats: Some(100),
            price_model: None,
            icon_url: None,
            website: None,
            recommendation_count: 0,
        };

        let filters = SearchFilters::new().free_only();
        assert!(!skill.matches_filters(&filters));

        let free_skill = SkillListing {
            price_sats: None,
            ..skill.clone()
        };
        assert!(free_skill.matches_filters(&filters));
    }

    #[test]
    fn test_skill_listing_matches_filters_max_price() {
        let skill = SkillListing {
            id: "test".to_string(),
            name: "Test".to_string(),
            description: "Test skill".to_string(),
            version: "1.0.0".to_string(),
            creator_pubkey: "creator".to_string(),
            capabilities: vec![],
            price_sats: Some(1000),
            price_model: None,
            icon_url: None,
            website: None,
            recommendation_count: 0,
        };

        let filters = SearchFilters::new().with_max_price(500);
        assert!(!skill.matches_filters(&filters));

        let filters = SearchFilters::new().with_max_price(1500);
        assert!(skill.matches_filters(&filters));
    }

    #[test]
    fn test_skill_listing_matches_filters_capability() {
        let skill = SkillListing {
            id: "test".to_string(),
            name: "Test".to_string(),
            description: "Test skill".to_string(),
            version: "1.0.0".to_string(),
            creator_pubkey: "creator".to_string(),
            capabilities: vec!["fetch".to_string(), "parse".to_string()],
            price_sats: None,
            price_model: None,
            icon_url: None,
            website: None,
            recommendation_count: 0,
        };

        let filters = SearchFilters::new().with_capability("fetch");
        assert!(skill.matches_filters(&filters));

        let filters = SearchFilters::new().with_capability("extract");
        assert!(!skill.matches_filters(&filters));
    }

    #[test]
    fn test_skill_listing_matches_filters_query() {
        let skill = SkillListing {
            id: "test".to_string(),
            name: "Web Scraper".to_string(),
            description: "Fetch and parse web pages".to_string(),
            version: "1.0.0".to_string(),
            creator_pubkey: "creator".to_string(),
            capabilities: vec![],
            price_sats: None,
            price_model: None,
            icon_url: None,
            website: None,
            recommendation_count: 0,
        };

        let filters = SearchFilters::new().with_query("scraper");
        assert!(skill.matches_filters(&filters));

        let filters = SearchFilters::new().with_query("fetch");
        assert!(skill.matches_filters(&filters));

        let filters = SearchFilters::new().with_query("database");
        assert!(!skill.matches_filters(&filters));
    }

    #[tokio::test]
    async fn test_skill_browser_empty() {
        let browser = SkillBrowser::new();
        let result = browser.browse(SearchFilters::new(), SortBy::Name).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 0);
    }
}
