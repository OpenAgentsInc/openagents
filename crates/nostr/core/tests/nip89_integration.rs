//! Integration tests for NIP-89 handler information and recommendations

use nostr::{
    HandlerInfo, HandlerMetadata, HandlerRecommendation, HandlerType, KIND_HANDLER_INFO,
    KIND_HANDLER_RECOMMENDATION, PricingInfo, SocialTrustScore,
};
use std::str::FromStr;

// =========================================================================
// HandlerType tests
// =========================================================================

#[test]
fn test_handler_type_serialization() {
    assert_eq!(HandlerType::Skill.as_str(), "skill");
    assert_eq!(HandlerType::Agent.as_str(), "agent");
    assert_eq!(HandlerType::ComputeProvider.as_str(), "compute_provider");
}

#[test]
fn test_handler_type_case_insensitive_parsing() {
    assert!(matches!(
        HandlerType::from_str("skill"),
        Ok(HandlerType::Skill)
    ));
    assert!(matches!(
        HandlerType::from_str("SKILL"),
        Ok(HandlerType::Skill)
    ));
    assert!(matches!(
        HandlerType::from_str("Skill"),
        Ok(HandlerType::Skill)
    ));
    assert!(matches!(
        HandlerType::from_str("agent"),
        Ok(HandlerType::Agent)
    ));
    assert!(matches!(
        HandlerType::from_str("AGENT"),
        Ok(HandlerType::Agent)
    ));
}

#[test]
fn test_handler_type_compute_provider_variants() {
    assert!(matches!(
        HandlerType::from_str("compute_provider"),
        Ok(HandlerType::ComputeProvider)
    ));
    assert!(matches!(
        HandlerType::from_str("computeprovider"),
        Ok(HandlerType::ComputeProvider)
    ));
    assert!(matches!(
        HandlerType::from_str("COMPUTE_PROVIDER"),
        Ok(HandlerType::ComputeProvider)
    ));
}

#[test]
fn test_handler_type_invalid() {
    assert!(HandlerType::from_str("invalid").is_err());
    assert!(HandlerType::from_str("").is_err());
    assert!(HandlerType::from_str("compute").is_err());
    assert!(HandlerType::from_str("provider").is_err());
}

// =========================================================================
// HandlerMetadata tests
// =========================================================================

#[test]
fn test_handler_metadata_minimal() {
    let metadata = HandlerMetadata::new("Simple Handler", "A basic handler");

    assert_eq!(metadata.name, "Simple Handler");
    assert_eq!(metadata.description, "A basic handler");
    assert!(metadata.icon_url.is_none());
    assert!(metadata.website.is_none());
}

#[test]
fn test_handler_metadata_with_icon() {
    let metadata =
        HandlerMetadata::new("Handler", "Description").with_icon("https://example.com/icon.png");

    assert_eq!(
        metadata.icon_url,
        Some("https://example.com/icon.png".to_string())
    );
}

#[test]
fn test_handler_metadata_with_website() {
    let metadata =
        HandlerMetadata::new("Handler", "Description").with_website("https://example.com");

    assert_eq!(metadata.website, Some("https://example.com".to_string()));
}

#[test]
fn test_handler_metadata_full() {
    let metadata = HandlerMetadata::new("OpenAgents Compute", "AI compute provider")
        .with_icon("https://openagents.com/icon.png")
        .with_website("https://openagents.com");

    assert_eq!(metadata.name, "OpenAgents Compute");
    assert_eq!(metadata.description, "AI compute provider");
    assert_eq!(
        metadata.icon_url,
        Some("https://openagents.com/icon.png".to_string())
    );
    assert_eq!(metadata.website, Some("https://openagents.com".to_string()));
}

// =========================================================================
// PricingInfo tests
// =========================================================================

#[test]
fn test_pricing_info_minimal() {
    let pricing = PricingInfo::new(1000);

    assert_eq!(pricing.amount, 1000);
    assert!(pricing.model.is_none());
    assert!(pricing.currency.is_none());
}

#[test]
fn test_pricing_info_with_model() {
    let pricing = PricingInfo::new(500).with_model("per-request");

    assert_eq!(pricing.amount, 500);
    assert_eq!(pricing.model, Some("per-request".to_string()));
}

#[test]
fn test_pricing_info_with_currency() {
    let pricing = PricingInfo::new(2000).with_currency("sats");

    assert_eq!(pricing.amount, 2000);
    assert_eq!(pricing.currency, Some("sats".to_string()));
}

#[test]
fn test_pricing_info_full() {
    let pricing = PricingInfo::new(1500)
        .with_model("per-token")
        .with_currency("msats");

    assert_eq!(pricing.amount, 1500);
    assert_eq!(pricing.model, Some("per-token".to_string()));
    assert_eq!(pricing.currency, Some("msats".to_string()));
}

#[test]
fn test_pricing_info_various_models() {
    let per_request = PricingInfo::new(1000).with_model("per-request");
    let per_token = PricingInfo::new(100).with_model("per-token");
    let per_minute = PricingInfo::new(5000).with_model("per-minute");

    assert_eq!(per_request.model, Some("per-request".to_string()));
    assert_eq!(per_token.model, Some("per-token".to_string()));
    assert_eq!(per_minute.model, Some("per-minute".to_string()));
}

// =========================================================================
// HandlerInfo tests
// =========================================================================

#[test]
fn test_handler_info_skill() {
    let metadata = HandlerMetadata::new("Code Generator", "Generates code from prompts");
    let info = HandlerInfo::new("npub1abc", HandlerType::Skill, metadata);

    assert_eq!(info.pubkey, "npub1abc");
    assert_eq!(info.handler_type, HandlerType::Skill);
    assert!(info.capabilities.is_empty());
    assert!(info.pricing.is_none());
}

#[test]
fn test_handler_info_agent() {
    let metadata = HandlerMetadata::new("AI Assistant", "Autonomous coding agent");
    let info = HandlerInfo::new("npub1def", HandlerType::Agent, metadata);

    assert_eq!(info.handler_type, HandlerType::Agent);
}

#[test]
fn test_handler_info_compute_provider() {
    let metadata = HandlerMetadata::new("GPU Compute", "High-performance GPU compute");
    let info = HandlerInfo::new("npub1ghi", HandlerType::ComputeProvider, metadata);

    assert_eq!(info.handler_type, HandlerType::ComputeProvider);
}

#[test]
fn test_handler_info_with_capabilities() {
    let metadata = HandlerMetadata::new("Multi-Skill Handler", "Versatile AI handler");
    let info = HandlerInfo::new("npub1xyz", HandlerType::Skill, metadata)
        .add_capability("code-generation")
        .add_capability("debugging")
        .add_capability("documentation")
        .add_capability("testing");

    assert_eq!(info.capabilities.len(), 4);
    assert!(info.capabilities.contains(&"code-generation".to_string()));
    assert!(info.capabilities.contains(&"debugging".to_string()));
}

#[test]
fn test_handler_info_with_pricing() {
    let metadata = HandlerMetadata::new("Paid Service", "Premium AI service");
    let pricing = PricingInfo::new(5000).with_model("per-request");
    let info = HandlerInfo::new("npub1paid", HandlerType::Agent, metadata).with_pricing(pricing);

    assert!(info.pricing.is_some());
    assert_eq!(info.pricing.as_ref().unwrap().amount, 5000);
}

#[test]
fn test_handler_info_to_tags_minimal() {
    let metadata = HandlerMetadata::new("Simple", "Simple handler");
    let info = HandlerInfo::new("npub1test", HandlerType::Skill, metadata);

    let tags = info.to_tags();

    // Should have at least the handler type tag
    assert!(tags.iter().any(|t| t[0] == "handler" && t[1] == "skill"));
}

#[test]
fn test_handler_info_to_tags_with_capabilities() {
    let metadata = HandlerMetadata::new("Handler", "Test handler");
    let info = HandlerInfo::new("npub1test", HandlerType::Agent, metadata)
        .add_capability("capability1")
        .add_capability("capability2")
        .add_capability("capability3");

    let tags = info.to_tags();

    // Check handler type tag
    assert!(tags.iter().any(|t| t[0] == "handler" && t[1] == "agent"));

    // Check capability tags
    assert!(
        tags.iter()
            .any(|t| t[0] == "capability" && t[1] == "capability1")
    );
    assert!(
        tags.iter()
            .any(|t| t[0] == "capability" && t[1] == "capability2")
    );
    assert!(
        tags.iter()
            .any(|t| t[0] == "capability" && t[1] == "capability3")
    );
}

#[test]
fn test_handler_info_to_tags_with_pricing_minimal() {
    let metadata = HandlerMetadata::new("Handler", "Test");
    let pricing = PricingInfo::new(1000);
    let info =
        HandlerInfo::new("npub1test", HandlerType::ComputeProvider, metadata).with_pricing(pricing);

    let tags = info.to_tags();

    // Check price tag with just amount
    assert!(tags.iter().any(|t| t[0] == "price" && t[1] == "1000"));
}

#[test]
fn test_handler_info_to_tags_with_pricing_full() {
    let metadata = HandlerMetadata::new("Handler", "Test");
    let pricing = PricingInfo::new(2500)
        .with_model("per-token")
        .with_currency("msats");
    let info = HandlerInfo::new("npub1test", HandlerType::Skill, metadata).with_pricing(pricing);

    let tags = info.to_tags();

    // Check price tag with amount, model, and currency
    assert!(tags.iter().any(|t| t.len() >= 4
        && t[0] == "price"
        && t[1] == "2500"
        && t[2] == "per-token"
        && t[3] == "msats"));
}

#[test]
fn test_handler_info_to_tags_comprehensive() {
    let metadata = HandlerMetadata::new("Full Handler", "Complete handler info")
        .with_icon("https://example.com/icon.png")
        .with_website("https://example.com");
    let pricing = PricingInfo::new(3000)
        .with_model("per-request")
        .with_currency("sats");
    let info = HandlerInfo::new("npub1full", HandlerType::Agent, metadata)
        .add_capability("cap1")
        .add_capability("cap2")
        .with_pricing(pricing);

    let tags = info.to_tags();

    // Verify all tag types are present
    assert!(tags.iter().any(|t| t[0] == "handler"));
    assert!(tags.iter().any(|t| t[0] == "capability"));
    assert!(tags.iter().any(|t| t[0] == "price"));
    assert!(tags.len() >= 4); // handler + 2 capabilities + price
}

// =========================================================================
// HandlerRecommendation tests
// =========================================================================

#[test]
fn test_handler_recommendation_minimal() {
    let rec = HandlerRecommendation::new("npub1recommender", "npub1handler");

    assert_eq!(rec.recommender, "npub1recommender");
    assert_eq!(rec.handler, "npub1handler");
    assert!(rec.rating.is_none());
    assert!(rec.comment.is_none());
}

#[test]
fn test_handler_recommendation_with_rating() {
    let rec = HandlerRecommendation::new("npub1rec", "npub1handler")
        .with_rating(5)
        .unwrap();

    assert_eq!(rec.rating, Some(5));
}

#[test]
fn test_handler_recommendation_valid_ratings() {
    for rating in 1..=5 {
        let rec = HandlerRecommendation::new("npub1rec", "npub1handler")
            .with_rating(rating)
            .unwrap();
        assert_eq!(rec.rating, Some(rating));
    }
}

#[test]
fn test_handler_recommendation_invalid_rating_too_low() {
    let result = HandlerRecommendation::new("npub1rec", "npub1handler").with_rating(0);
    assert!(result.is_err());
}

#[test]
fn test_handler_recommendation_invalid_rating_too_high() {
    let result = HandlerRecommendation::new("npub1rec", "npub1handler").with_rating(6);
    assert!(result.is_err());
}

#[test]
fn test_handler_recommendation_with_comment() {
    let rec = HandlerRecommendation::new("npub1rec", "npub1handler")
        .with_comment("Excellent service, very responsive!");

    assert_eq!(
        rec.comment,
        Some("Excellent service, very responsive!".to_string())
    );
}

#[test]
fn test_handler_recommendation_full() {
    let rec = HandlerRecommendation::new("npub1rec", "npub1handler")
        .with_rating(4)
        .unwrap()
        .with_comment("Great handler, minor issues");

    assert_eq!(rec.rating, Some(4));
    assert_eq!(rec.comment, Some("Great handler, minor issues".to_string()));
}

#[test]
fn test_handler_recommendation_to_tags_minimal() {
    let rec = HandlerRecommendation::new("npub1rec", "npub1handler");

    let tags = rec.to_tags();

    // Should have p tag for handler pubkey
    assert!(tags.iter().any(|t| t[0] == "p" && t[1] == "npub1handler"));
}

#[test]
fn test_handler_recommendation_to_tags_with_rating() {
    let rec = HandlerRecommendation::new("npub1rec", "npub1handler")
        .with_rating(5)
        .unwrap();

    let tags = rec.to_tags();

    // Check for p tag and rating tag
    assert!(tags.iter().any(|t| t[0] == "p" && t[1] == "npub1handler"));
    assert!(tags.iter().any(|t| t[0] == "rating" && t[1] == "5"));
}

#[test]
fn test_handler_recommendation_to_tags_all_ratings() {
    for rating in 1..=5 {
        let rec = HandlerRecommendation::new("npub1rec", "npub1handler")
            .with_rating(rating)
            .unwrap();
        let tags = rec.to_tags();
        assert!(
            tags.iter()
                .any(|t| t[0] == "rating" && t[1] == rating.to_string())
        );
    }
}

// =========================================================================
// SocialTrustScore tests
// =========================================================================

#[test]
fn test_social_trust_score_initialization() {
    let score = SocialTrustScore::new("npub1handler");

    assert_eq!(score.handler_id, "npub1handler");
    assert_eq!(score.trust_score, 0.0);
    assert_eq!(score.direct_follows, 0);
    assert_eq!(score.follow_of_follows, 0);
    assert_eq!(score.two_degrees, 0);
    assert_eq!(score.unknown, 0);
}

#[test]
fn test_social_trust_score_direct_follow_single() {
    let mut score = SocialTrustScore::new("npub1handler");
    score.add_direct_follow();

    assert_eq!(score.direct_follows, 1);
    assert!(score.trust_score > 0.0);
}

#[test]
fn test_social_trust_score_direct_follows_multiple() {
    let mut score = SocialTrustScore::new("npub1handler");
    for _ in 0..5 {
        score.add_direct_follow();
    }

    assert_eq!(score.direct_follows, 5);
    assert!(score.trust_score > 0.0);
}

#[test]
fn test_social_trust_score_follow_of_follow() {
    let mut score = SocialTrustScore::new("npub1handler");
    score.add_follow_of_follow();

    assert_eq!(score.follow_of_follows, 1);
    assert!(score.trust_score > 0.0);
}

#[test]
fn test_social_trust_score_two_degrees() {
    let mut score = SocialTrustScore::new("npub1handler");
    score.add_two_degrees();

    assert_eq!(score.two_degrees, 1);
    assert!(score.trust_score > 0.0);
}

#[test]
fn test_social_trust_score_unknown() {
    let mut score = SocialTrustScore::new("npub1handler");
    score.add_unknown();

    assert_eq!(score.unknown, 1);
    assert!(score.trust_score > 0.0);
}

#[test]
fn test_social_trust_score_mixed_recommendations() {
    let mut score = SocialTrustScore::new("npub1handler");
    score.add_direct_follow();
    score.add_direct_follow();
    score.add_follow_of_follow();
    score.add_two_degrees();
    score.add_unknown();

    assert_eq!(score.direct_follows, 2);
    assert_eq!(score.follow_of_follows, 1);
    assert_eq!(score.two_degrees, 1);
    assert_eq!(score.unknown, 1);
    assert!(score.trust_score > 0.0);
    assert!(score.trust_score < 1.0);
}

#[test]
fn test_social_trust_score_weighting_direct_vs_follow() {
    let mut score_direct = SocialTrustScore::new("handler1");
    score_direct.add_direct_follow();

    let mut score_follow = SocialTrustScore::new("handler2");
    score_follow.add_follow_of_follow();

    // Direct follows should have higher weight (1.0 vs 0.5)
    assert!(score_direct.trust_score > score_follow.trust_score);
}

#[test]
fn test_social_trust_score_weighting_follow_vs_two_degrees() {
    let mut score_follow = SocialTrustScore::new("handler1");
    score_follow.add_follow_of_follow();

    let mut score_two = SocialTrustScore::new("handler2");
    score_two.add_two_degrees();

    // Follow-of-follows should have higher weight (0.5 vs 0.25)
    assert!(score_follow.trust_score > score_two.trust_score);
}

#[test]
fn test_social_trust_score_weighting_two_degrees_vs_unknown() {
    let mut score_two = SocialTrustScore::new("handler1");
    score_two.add_two_degrees();

    let mut score_unknown = SocialTrustScore::new("handler2");
    score_unknown.add_unknown();

    // Two degrees should have higher weight (0.25 vs 0.1)
    assert!(score_two.trust_score > score_unknown.trust_score);
}

#[test]
fn test_social_trust_score_multiple_same_type() {
    let mut score = SocialTrustScore::new("npub1handler");
    for _ in 0..10 {
        score.add_direct_follow();
    }

    assert_eq!(score.direct_follows, 10);
    assert!(score.trust_score > 0.0);
}

#[test]
fn test_social_trust_score_normalization() {
    let mut score = SocialTrustScore::new("npub1handler");

    // Add many recommendations
    for _ in 0..100 {
        score.add_direct_follow();
    }

    // Trust score should be normalized to less than 1.0
    assert!(score.trust_score < 1.0);
}

#[test]
fn test_social_trust_score_incremental_calculation() {
    let mut score = SocialTrustScore::new("npub1handler");

    score.add_direct_follow();
    let score1 = score.trust_score;

    score.add_direct_follow();
    let score2 = score.trust_score;

    score.add_follow_of_follow();
    let score3 = score.trust_score;

    // Score should increase with each addition
    assert!(score2 > score1);
    assert!(score3 > score2);
}

// =========================================================================
// Integration workflow tests
// =========================================================================

#[test]
fn test_compute_provider_advertisement_workflow() {
    // Create a compute provider handler info
    let metadata = HandlerMetadata::new("GPU Compute Farm", "High-performance GPU inference")
        .with_icon("https://compute.example.com/icon.png")
        .with_website("https://compute.example.com");

    let pricing = PricingInfo::new(5000)
        .with_model("per-minute")
        .with_currency("sats");

    let handler_info = HandlerInfo::new("npub1compute", HandlerType::ComputeProvider, metadata)
        .add_capability("text-generation")
        .add_capability("image-generation")
        .add_capability("video-processing")
        .with_pricing(pricing);

    // Verify handler info structure
    assert_eq!(handler_info.handler_type, HandlerType::ComputeProvider);
    assert_eq!(handler_info.capabilities.len(), 3);
    assert!(handler_info.pricing.is_some());

    // Verify tags can be generated
    let tags = handler_info.to_tags();
    assert!(!tags.is_empty());
    assert!(
        tags.iter()
            .any(|t| t[0] == "handler" && t[1] == "compute_provider")
    );
}

#[test]
fn test_skill_handler_workflow() {
    let metadata = HandlerMetadata::new("Code Review Skill", "Automated code review");
    let handler_info = HandlerInfo::new("npub1skill", HandlerType::Skill, metadata)
        .add_capability("rust-review")
        .add_capability("javascript-review")
        .add_capability("security-audit");

    assert_eq!(handler_info.handler_type, HandlerType::Skill);
    assert_eq!(handler_info.capabilities.len(), 3);
}

#[test]
fn test_agent_recommendation_workflow() {
    // User recommends an agent they've used
    let recommendation = HandlerRecommendation::new("npub1user", "npub1agent")
        .with_rating(5)
        .unwrap()
        .with_comment("This agent helped me debug a complex issue in minutes!");

    let tags = recommendation.to_tags();
    assert!(tags.iter().any(|t| t[0] == "p" && t[1] == "npub1agent"));
    assert!(tags.iter().any(|t| t[0] == "rating" && t[1] == "5"));
}

#[test]
fn test_social_discovery_workflow() {
    let mut score = SocialTrustScore::new("npub1handler");

    // Simulate social graph recommendations
    // 3 direct follows recommend
    score.add_direct_follow();
    score.add_direct_follow();
    score.add_direct_follow();

    // 5 follow-of-follows recommend
    for _ in 0..5 {
        score.add_follow_of_follow();
    }

    // 2 at two degrees
    score.add_two_degrees();
    score.add_two_degrees();

    // 1 unknown
    score.add_unknown();

    // Verify counts
    assert_eq!(score.direct_follows, 3);
    assert_eq!(score.follow_of_follows, 5);
    assert_eq!(score.two_degrees, 2);
    assert_eq!(score.unknown, 1);

    // Trust score should be meaningful
    assert!(score.trust_score > 0.0);
    assert!(score.trust_score < 1.0);
}

#[test]
fn test_free_vs_paid_handlers() {
    let metadata_free = HandlerMetadata::new("Free Handler", "Free service");
    let free_handler = HandlerInfo::new("npub1free", HandlerType::Skill, metadata_free);

    let metadata_paid = HandlerMetadata::new("Paid Handler", "Premium service");
    let pricing = PricingInfo::new(10000).with_model("per-request");
    let paid_handler =
        HandlerInfo::new("npub1paid", HandlerType::Skill, metadata_paid).with_pricing(pricing);

    // Free handler has no pricing
    assert!(free_handler.pricing.is_none());

    // Paid handler has pricing
    assert!(paid_handler.pricing.is_some());
    assert_eq!(paid_handler.pricing.unwrap().amount, 10000);
}

#[test]
fn test_multi_capability_handler() {
    let metadata = HandlerMetadata::new("Multi-Purpose AI", "Versatile AI agent");
    let handler = HandlerInfo::new("npub1multi", HandlerType::Agent, metadata)
        .add_capability("code-generation")
        .add_capability("code-review")
        .add_capability("documentation")
        .add_capability("testing")
        .add_capability("debugging")
        .add_capability("refactoring");

    assert_eq!(handler.capabilities.len(), 6);

    let tags = handler.to_tags();
    let capability_tags: Vec<_> = tags.iter().filter(|t| t[0] == "capability").collect();
    assert_eq!(capability_tags.len(), 6);
}

#[test]
fn test_handler_kinds_validation() {
    assert_eq!(KIND_HANDLER_INFO, 31990);
    assert_eq!(KIND_HANDLER_RECOMMENDATION, 31989);
}
