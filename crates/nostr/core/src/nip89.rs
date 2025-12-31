//! NIP-89: Application Handlers
//!
//! This NIP defines a way for clients to discover and recommend application handlers
//! (agents, skills, compute providers) based on social graph trust.
//!
//! ## Kinds
//! - 31990: Handler information (advertise capabilities)
//! - 31989: Handler recommendation (social discovery)
//!
//! ## Social Discovery
//! Handler recommendations are weighted by social graph distance:
//! - Direct follows: weight 1.0
//! - Follow-of-follows: weight 0.5
//! - Two degrees separation: weight 0.25
//! - Unknown: weight 0.1

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Kind for handler information
pub const KIND_HANDLER_INFO: u16 = 31990;

/// Kind for handler recommendation
pub const KIND_HANDLER_RECOMMENDATION: u16 = 31989;

/// Errors that can occur during NIP-89 operations.
#[derive(Debug, Error)]
pub enum Nip89Error {
    #[error("invalid kind: {0} (expected {1})")]
    InvalidKind(u16, String),

    #[error("missing required tag: {0}")]
    MissingTag(String),

    #[error("invalid handler type: {0}")]
    InvalidHandlerType(String),

    #[error("invalid rating: {0} (expected 1-5)")]
    InvalidRating(u8),

    #[error("serialization error: {0}")]
    Serialization(String),
}

/// Type of handler advertised in kind 31990.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HandlerType {
    /// A skill/capability handler
    Skill,
    /// An autonomous agent
    Agent,
    /// A compute provider
    ComputeProvider,
}

impl HandlerType {
    pub fn as_str(&self) -> &'static str {
        match self {
            HandlerType::Skill => "skill",
            HandlerType::Agent => "agent",
            HandlerType::ComputeProvider => "compute_provider",
        }
    }

    pub fn from_str(s: &str) -> Result<Self, Nip89Error> {
        match s.to_lowercase().as_str() {
            "skill" => Ok(HandlerType::Skill),
            "agent" => Ok(HandlerType::Agent),
            "compute_provider" | "computeprovider" => Ok(HandlerType::ComputeProvider),
            _ => Err(Nip89Error::InvalidHandlerType(s.to_string())),
        }
    }
}

/// Metadata for a handler.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HandlerMetadata {
    /// Human-readable name
    pub name: String,
    /// Description of capabilities
    pub description: String,
    /// Optional icon URL
    pub icon_url: Option<String>,
    /// Optional website
    pub website: Option<String>,
}

impl HandlerMetadata {
    pub fn new(name: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            icon_url: None,
            website: None,
        }
    }

    pub fn with_icon(mut self, url: impl Into<String>) -> Self {
        self.icon_url = Some(url.into());
        self
    }

    pub fn with_website(mut self, url: impl Into<String>) -> Self {
        self.website = Some(url.into());
        self
    }
}

/// Pricing information for a handler.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PricingInfo {
    /// Price in millisats
    pub amount: u64,
    /// Optional pricing model (e.g., "per-request", "per-token", "per-minute")
    pub model: Option<String>,
    /// Optional currency (defaults to sats)
    pub currency: Option<String>,
}

impl PricingInfo {
    pub fn new(amount: u64) -> Self {
        Self {
            amount,
            model: None,
            currency: None,
        }
    }

    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }

    pub fn with_currency(mut self, currency: impl Into<String>) -> Self {
        self.currency = Some(currency.into());
        self
    }
}

/// Handler information event data (kind 31990).
#[derive(Debug, Clone)]
pub struct HandlerInfo {
    /// Handler's pubkey
    pub pubkey: String,
    /// Type of handler
    pub handler_type: HandlerType,
    /// List of capabilities
    pub capabilities: Vec<String>,
    /// Optional pricing information
    pub pricing: Option<PricingInfo>,
    /// Handler metadata
    pub metadata: HandlerMetadata,
    /// Custom tags (e.g., region, availability)
    pub custom_tags: Vec<(String, String)>,
}

impl HandlerInfo {
    /// Create a new handler info.
    pub fn new(
        pubkey: impl Into<String>,
        handler_type: HandlerType,
        metadata: HandlerMetadata,
    ) -> Self {
        Self {
            pubkey: pubkey.into(),
            handler_type,
            capabilities: Vec::new(),
            pricing: None,
            metadata,
            custom_tags: Vec::new(),
        }
    }

    /// Add a capability.
    pub fn add_capability(mut self, capability: impl Into<String>) -> Self {
        self.capabilities.push(capability.into());
        self
    }

    /// Set pricing information.
    pub fn with_pricing(mut self, pricing: PricingInfo) -> Self {
        self.pricing = Some(pricing);
        self
    }

    /// Add a custom tag (key-value pair).
    pub fn add_custom_tag(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.custom_tags.push((key.into(), value.into()));
        self
    }

    /// Convert to tags for event creation.
    pub fn to_tags(&self) -> Vec<Vec<String>> {
        let mut tags = Vec::new();

        // Add handler type
        tags.push(vec![
            "handler".to_string(),
            self.handler_type.as_str().to_string(),
        ]);

        // Add capabilities
        for capability in &self.capabilities {
            tags.push(vec!["capability".to_string(), capability.clone()]);
        }

        // Add pricing if present
        if let Some(pricing) = &self.pricing {
            let mut price_tag = vec!["price".to_string(), pricing.amount.to_string()];
            if let Some(model) = &pricing.model {
                price_tag.push(model.clone());
            }
            if let Some(currency) = &pricing.currency {
                price_tag.push(currency.clone());
            }
            tags.push(price_tag);
        }

        // Add custom tags
        for (key, value) in &self.custom_tags {
            tags.push(vec![key.clone(), value.clone()]);
        }

        tags
    }

    /// Parse a HandlerInfo from a Nostr event.
    pub fn from_event(event: &crate::Event) -> Result<Self, Nip89Error> {
        if event.kind != KIND_HANDLER_INFO {
            return Err(Nip89Error::InvalidKind(event.kind, "31990".to_string()));
        }

        // Parse handler type from tags
        let handler_type = event
            .tags
            .iter()
            .find(|t| t.len() >= 2 && t[0] == "handler")
            .map(|t| HandlerType::from_str(&t[1]))
            .transpose()?
            .ok_or_else(|| Nip89Error::MissingTag("handler".to_string()))?;

        // Parse capabilities from tags
        let capabilities: Vec<String> = event
            .tags
            .iter()
            .filter(|t| t.len() >= 2 && t[0] == "capability")
            .map(|t| t[1].clone())
            .collect();

        // Parse pricing from tags
        let pricing = event
            .tags
            .iter()
            .find(|t| t.len() >= 2 && t[0] == "price")
            .and_then(|t| t[1].parse::<u64>().ok())
            .map(|amount| {
                let tag = event
                    .tags
                    .iter()
                    .find(|t| t.len() >= 2 && t[0] == "price")
                    .unwrap();
                let mut pricing = PricingInfo::new(amount);
                if tag.len() >= 3 {
                    pricing = pricing.with_model(tag[2].clone());
                }
                if tag.len() >= 4 {
                    pricing = pricing.with_currency(tag[3].clone());
                }
                pricing
            });

        // Parse metadata from content (JSON)
        let metadata: HandlerMetadata = serde_json::from_str(&event.content)
            .map_err(|e| Nip89Error::Serialization(e.to_string()))?;

        // Parse custom tags (everything except known tag types)
        let known_tags = [
            "handler",
            "capability",
            "price",
            "p",
            "e",
            "a",
            "d",
            "rating",
        ];
        let custom_tags: Vec<(String, String)> = event
            .tags
            .iter()
            .filter(|t| t.len() >= 2 && !known_tags.contains(&t[0].as_str()))
            .map(|t| (t[0].clone(), t[1].clone()))
            .collect();

        Ok(Self {
            pubkey: event.pubkey.clone(),
            handler_type,
            capabilities,
            pricing,
            metadata,
            custom_tags,
        })
    }
}

/// Handler recommendation event data (kind 31989).
#[derive(Debug, Clone)]
pub struct HandlerRecommendation {
    /// Recommender's pubkey
    pub recommender: String,
    /// Handler's pubkey being recommended
    pub handler: String,
    /// Optional rating (1-5)
    pub rating: Option<u8>,
    /// Optional comment
    pub comment: Option<String>,
}

impl HandlerRecommendation {
    /// Create a new handler recommendation.
    pub fn new(recommender: impl Into<String>, handler: impl Into<String>) -> Self {
        Self {
            recommender: recommender.into(),
            handler: handler.into(),
            rating: None,
            comment: None,
        }
    }

    /// Set a rating (1-5).
    pub fn with_rating(mut self, rating: u8) -> Result<Self, Nip89Error> {
        if !(1..=5).contains(&rating) {
            return Err(Nip89Error::InvalidRating(rating));
        }
        self.rating = Some(rating);
        Ok(self)
    }

    /// Set a comment.
    pub fn with_comment(mut self, comment: impl Into<String>) -> Self {
        self.comment = Some(comment.into());
        self
    }

    /// Convert to tags for event creation.
    pub fn to_tags(&self) -> Vec<Vec<String>> {
        let mut tags = Vec::new();

        // Add handler pubkey reference
        tags.push(vec!["p".to_string(), self.handler.clone()]);

        // Add rating if present
        if let Some(rating) = self.rating {
            tags.push(vec!["rating".to_string(), rating.to_string()]);
        }

        tags
    }
}

/// Social trust score for a handler based on social graph distance.
#[derive(Debug, Clone, PartialEq)]
pub struct SocialTrustScore {
    /// Handler's pubkey
    pub handler_id: String,
    /// Calculated trust score (0.0-1.0)
    pub trust_score: f32,
    /// Number of direct follows recommending
    pub direct_follows: u32,
    /// Number of follow-of-follows recommending
    pub follow_of_follows: u32,
    /// Number at two degrees separation
    pub two_degrees: u32,
    /// Number of unknown recommenders
    pub unknown: u32,
}

impl SocialTrustScore {
    /// Create a new social trust score.
    pub fn new(handler_id: impl Into<String>) -> Self {
        Self {
            handler_id: handler_id.into(),
            trust_score: 0.0,
            direct_follows: 0,
            follow_of_follows: 0,
            two_degrees: 0,
            unknown: 0,
        }
    }

    /// Calculate the trust score based on social graph distances.
    ///
    /// Weights:
    /// - Direct follows: 1.0
    /// - Follow-of-follows: 0.5
    /// - Two degrees: 0.25
    /// - Unknown: 0.1
    pub fn calculate(&mut self) {
        let weighted_sum = (self.direct_follows as f32 * 1.0)
            + (self.follow_of_follows as f32 * 0.5)
            + (self.two_degrees as f32 * 0.25)
            + (self.unknown as f32 * 0.1);

        let total_recommendations =
            self.direct_follows + self.follow_of_follows + self.two_degrees + self.unknown;

        // Normalize to 0.0-1.0 range
        if total_recommendations > 0 {
            // Use a sigmoid-like function to normalize
            self.trust_score = weighted_sum / (weighted_sum + 10.0);
        } else {
            self.trust_score = 0.0;
        }
    }

    /// Add a recommendation from a direct follow.
    pub fn add_direct_follow(&mut self) {
        self.direct_follows += 1;
        self.calculate();
    }

    /// Add a recommendation from a follow-of-follow.
    pub fn add_follow_of_follow(&mut self) {
        self.follow_of_follows += 1;
        self.calculate();
    }

    /// Add a recommendation from two degrees separation.
    pub fn add_two_degrees(&mut self) {
        self.two_degrees += 1;
        self.calculate();
    }

    /// Add a recommendation from an unknown user.
    pub fn add_unknown(&mut self) {
        self.unknown += 1;
        self.calculate();
    }
}

/// Check if a kind is a handler info kind (31990).
pub fn is_handler_info_kind(kind: u16) -> bool {
    kind == KIND_HANDLER_INFO
}

/// Check if a kind is a handler recommendation kind (31989).
pub fn is_handler_recommendation_kind(kind: u16) -> bool {
    kind == KIND_HANDLER_RECOMMENDATION
}

/// Check if a kind is any NIP-89 kind.
pub fn is_nip89_kind(kind: u16) -> bool {
    is_handler_info_kind(kind) || is_handler_recommendation_kind(kind)
}

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // Kind validation tests
    // =========================================================================

    #[test]
    fn test_is_handler_info_kind() {
        assert!(is_handler_info_kind(31990));
        assert!(!is_handler_info_kind(31989));
        assert!(!is_handler_info_kind(1));
    }

    #[test]
    fn test_is_handler_recommendation_kind() {
        assert!(is_handler_recommendation_kind(31989));
        assert!(!is_handler_recommendation_kind(31990));
        assert!(!is_handler_recommendation_kind(1));
    }

    #[test]
    fn test_is_nip89_kind() {
        assert!(is_nip89_kind(31989));
        assert!(is_nip89_kind(31990));
        assert!(!is_nip89_kind(1));
        assert!(!is_nip89_kind(5000));
    }

    // =========================================================================
    // HandlerType tests
    // =========================================================================

    #[test]
    fn test_handler_type_as_str() {
        assert_eq!(HandlerType::Skill.as_str(), "skill");
        assert_eq!(HandlerType::Agent.as_str(), "agent");
        assert_eq!(HandlerType::ComputeProvider.as_str(), "compute_provider");
    }

    #[test]
    fn test_handler_type_from_str() {
        assert_eq!(HandlerType::from_str("skill").unwrap(), HandlerType::Skill);
        assert_eq!(HandlerType::from_str("agent").unwrap(), HandlerType::Agent);
        assert_eq!(
            HandlerType::from_str("compute_provider").unwrap(),
            HandlerType::ComputeProvider
        );

        // Case insensitive
        assert_eq!(HandlerType::from_str("SKILL").unwrap(), HandlerType::Skill);
        assert_eq!(HandlerType::from_str("Agent").unwrap(), HandlerType::Agent);

        // Invalid
        assert!(HandlerType::from_str("invalid").is_err());
    }

    // =========================================================================
    // HandlerMetadata tests
    // =========================================================================

    #[test]
    fn test_handler_metadata() {
        let metadata =
            HandlerMetadata::new("Code Generator", "Generates code from natural language")
                .with_icon("https://example.com/icon.png")
                .with_website("https://example.com");

        assert_eq!(metadata.name, "Code Generator");
        assert_eq!(metadata.description, "Generates code from natural language");
        assert_eq!(
            metadata.icon_url,
            Some("https://example.com/icon.png".to_string())
        );
        assert_eq!(metadata.website, Some("https://example.com".to_string()));
    }

    // =========================================================================
    // PricingInfo tests
    // =========================================================================

    #[test]
    fn test_pricing_info() {
        let pricing = PricingInfo::new(1000)
            .with_model("per-request")
            .with_currency("sats");

        assert_eq!(pricing.amount, 1000);
        assert_eq!(pricing.model, Some("per-request".to_string()));
        assert_eq!(pricing.currency, Some("sats".to_string()));
    }

    // =========================================================================
    // HandlerInfo tests
    // =========================================================================

    #[test]
    fn test_handler_info() {
        let metadata = HandlerMetadata::new("AI Assistant", "Helps with coding tasks");
        let info = HandlerInfo::new("pubkey123", HandlerType::Agent, metadata)
            .add_capability("code-generation")
            .add_capability("debugging")
            .with_pricing(PricingInfo::new(500).with_model("per-request"));

        assert_eq!(info.pubkey, "pubkey123");
        assert_eq!(info.handler_type, HandlerType::Agent);
        assert_eq!(info.capabilities.len(), 2);
        assert!(info.pricing.is_some());
    }

    #[test]
    fn test_handler_info_to_tags() {
        let metadata = HandlerMetadata::new("Test Handler", "Description");
        let info = HandlerInfo::new("pubkey123", HandlerType::Skill, metadata)
            .add_capability("skill1")
            .add_capability("skill2")
            .with_pricing(PricingInfo::new(1000));

        let tags = info.to_tags();

        assert!(tags.iter().any(|t| t[0] == "handler" && t[1] == "skill"));
        assert!(
            tags.iter()
                .any(|t| t[0] == "capability" && t[1] == "skill1")
        );
        assert!(
            tags.iter()
                .any(|t| t[0] == "capability" && t[1] == "skill2")
        );
        assert!(tags.iter().any(|t| t[0] == "price" && t[1] == "1000"));
    }

    // =========================================================================
    // HandlerRecommendation tests
    // =========================================================================

    #[test]
    fn test_handler_recommendation() {
        let rec = HandlerRecommendation::new("recommender123", "handler456")
            .with_rating(5)
            .unwrap()
            .with_comment("Excellent service!");

        assert_eq!(rec.recommender, "recommender123");
        assert_eq!(rec.handler, "handler456");
        assert_eq!(rec.rating, Some(5));
        assert_eq!(rec.comment, Some("Excellent service!".to_string()));
    }

    #[test]
    fn test_handler_recommendation_invalid_rating() {
        let rec = HandlerRecommendation::new("recommender123", "handler456").with_rating(6);
        assert!(rec.is_err());

        let rec = HandlerRecommendation::new("recommender123", "handler456").with_rating(0);
        assert!(rec.is_err());
    }

    #[test]
    fn test_handler_recommendation_to_tags() {
        let rec = HandlerRecommendation::new("recommender123", "handler456")
            .with_rating(4)
            .unwrap();

        let tags = rec.to_tags();

        assert!(tags.iter().any(|t| t[0] == "p" && t[1] == "handler456"));
        assert!(tags.iter().any(|t| t[0] == "rating" && t[1] == "4"));
    }

    // =========================================================================
    // SocialTrustScore tests
    // =========================================================================

    #[test]
    fn test_social_trust_score_empty() {
        let score = SocialTrustScore::new("handler123");
        assert_eq!(score.handler_id, "handler123");
        assert_eq!(score.trust_score, 0.0);
        assert_eq!(score.direct_follows, 0);
    }

    #[test]
    fn test_social_trust_score_direct_follows() {
        let mut score = SocialTrustScore::new("handler123");
        score.add_direct_follow();
        score.add_direct_follow();

        assert_eq!(score.direct_follows, 2);
        assert!(score.trust_score > 0.0);
    }

    #[test]
    fn test_social_trust_score_mixed() {
        let mut score = SocialTrustScore::new("handler123");
        score.add_direct_follow();
        score.add_follow_of_follow();
        score.add_two_degrees();
        score.add_unknown();

        assert_eq!(score.direct_follows, 1);
        assert_eq!(score.follow_of_follows, 1);
        assert_eq!(score.two_degrees, 1);
        assert_eq!(score.unknown, 1);
        assert!(score.trust_score > 0.0);
        assert!(score.trust_score < 1.0);
    }

    #[test]
    fn test_social_trust_score_ordering() {
        let mut score1 = SocialTrustScore::new("handler1");
        score1.add_direct_follow();
        score1.add_direct_follow();

        let mut score2 = SocialTrustScore::new("handler2");
        score2.add_follow_of_follow();
        score2.add_follow_of_follow();

        let mut score3 = SocialTrustScore::new("handler3");
        score3.add_unknown();
        score3.add_unknown();

        // Direct follows should have higher score than follow-of-follows
        assert!(score1.trust_score > score2.trust_score);
        // Follow-of-follows should have higher score than unknown
        assert!(score2.trust_score > score3.trust_score);
    }
}
