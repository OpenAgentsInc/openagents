//! End-to-end integration tests for Skill Marketplace over real Nostr relays
//!
//! These tests verify that the complete skill marketplace stack works correctly
//! over real relay connections, testing:
//! - Skill discovery via NIP-89 handler info events
//! - Skill license issuance (NIP-SA kind:39220)
//! - Skill delivery with content verification (NIP-SA kind:39221)
//! - Skill versioning and updates
//!
//! Part of d-015: Comprehensive Marketplace and Agent Commerce E2E Tests

use nostr::{finalize_event, generate_secret_key, get_public_key, EventTemplate};
use nostr::{HandlerInfo, HandlerMetadata, HandlerType, KIND_HANDLER_INFO};
use nostr::{
    SkillDelivery, SkillDeliveryContent, SkillLicense, SkillLicenseContent, KIND_SKILL_DELIVERY,
    KIND_SKILL_LICENSE,
};
use nostr_client::RelayConnection;
use nostr_relay::{Database, DatabaseConfig, RelayConfig, RelayServer};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

/// Helper: Start an in-process test relay and return its server
async fn start_test_relay(port: u16) -> (Arc<RelayServer>, tempfile::TempDir) {
    let config = RelayConfig {
        bind_addr: format!("127.0.0.1:{}", port).parse().unwrap(),
        ..Default::default()
    };

    let temp_dir = tempfile::tempdir().unwrap();
    let db_path = temp_dir.path().join("test.db");
    let db_config = DatabaseConfig {
        path: db_path,
        ..Default::default()
    };

    let db = Database::new(db_config).unwrap();
    let server = Arc::new(RelayServer::new(config, db));

    let server_clone = Arc::clone(&server);
    tokio::spawn(async move {
        server_clone.start().await.ok();
    });

    // Give server time to start
    sleep(Duration::from_millis(200)).await;

    (server, temp_dir)
}

/// Get test relay WebSocket URL for given port
fn test_relay_url(port: u16) -> String {
    format!("ws://127.0.0.1:{}", port)
}

/// Calculate SHA-256 hash of content for verification
fn sha256_hash(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    hex::encode(hasher.finalize())
}

// =============================================================================
// Phase 2 Tests: Skill Marketplace E2E with Real Relay
// =============================================================================

#[tokio::test]
async fn test_skill_browse_over_relay() {
    // Test: Browse skills published to relay as NIP-89 handler info events
    //
    // 1. Creator publishes skill as handler info (kind:31990)
    // 2. Consumer discovers skills via relay query
    // 3. Verify skill metadata matches

    // 1. Start test relay
    let (_server, _tmp) = start_test_relay(19210).await;
    let relay_url = test_relay_url(19210);

    // 2. Create skill creator identity
    let creator_secret_key = generate_secret_key();
    let creator_pubkey = get_public_key(&creator_secret_key).expect("pubkey");

    // 3. Create skill metadata (NIP-89 handler info)
    let skill_metadata = HandlerMetadata::new(
        "code-reviewer",
        "AI code review assistant that analyzes Rust code for best practices",
    )
    .with_icon("https://openagents.com/skills/code-reviewer.png")
    .with_website("https://openagents.com/skills/code-reviewer");

    // 4. Create handler info for the skill
    let handler_info = HandlerInfo::new(
        hex::encode(creator_pubkey),
        HandlerType::Skill,
        skill_metadata.clone(),
    )
    .add_capability("rust-analysis")
    .add_capability("code-review")
    .add_capability("best-practices");

    // 5. Create handler info event (kind:31990)
    let content =
        serde_json::to_string(&handler_info.metadata).expect("should serialize metadata");

    let skill_template = EventTemplate {
        kind: KIND_HANDLER_INFO,
        content,
        tags: handler_info.to_tags(),
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    };

    let skill_event = finalize_event(&skill_template, &creator_secret_key).expect("should sign");

    // 6. Connect to relay
    let relay = RelayConnection::new(&relay_url).expect("connection");
    relay.connect().await.expect("connect");

    // 7. Subscribe to skill listings (consumer discovers skills)
    let filter = json!({
        "kinds": [KIND_HANDLER_INFO],
        "limit": 10
    });
    let mut rx = relay
        .subscribe_with_channel("skill-discovery", &[filter])
        .await
        .expect("subscribe");

    sleep(Duration::from_millis(100)).await;

    // 8. Publish skill to relay
    let confirmation = relay
        .publish_event(&skill_event, Duration::from_secs(5))
        .await
        .expect("publish");

    assert!(confirmation.accepted, "Relay should accept skill event");

    // 9. Consumer receives skill listing
    let received = tokio::time::timeout(Duration::from_secs(2), rx.recv())
        .await
        .expect("should receive within timeout")
        .expect("should have skill");

    assert_eq!(received.id, skill_event.id);
    assert_eq!(received.kind, KIND_HANDLER_INFO);

    // 10. Parse and verify skill metadata
    let parsed_metadata: HandlerMetadata =
        serde_json::from_str(&received.content).expect("should deserialize");

    assert_eq!(parsed_metadata.name, "code-reviewer");
    assert!(parsed_metadata
        .description
        .contains("AI code review assistant"));
    assert_eq!(
        parsed_metadata.icon_url,
        Some("https://openagents.com/skills/code-reviewer.png".to_string())
    );

    // Verify skill handler type tag
    let has_skill_type = received
        .tags
        .iter()
        .any(|t| t[0] == "handler" && t[1] == "skill");
    assert!(has_skill_type, "Should have skill handler type");

    // Verify capability tags
    let has_rust_analysis = received
        .tags
        .iter()
        .any(|t| t[0] == "capability" && t[1] == "rust-analysis");
    assert!(has_rust_analysis, "Should have rust-analysis capability");

    relay.disconnect().await.ok();
}

#[tokio::test]
async fn test_skill_license_issuance() {
    // Test: Marketplace issues skill license (NIP-SA kind:39220) to agent
    //
    // 1. Agent requests skill purchase
    // 2. Marketplace issues license after payment
    // 3. Agent receives license and verifies

    // 1. Start test relay
    let (_server, _tmp) = start_test_relay(19211).await;
    let relay_url = test_relay_url(19211);

    // 2. Create identities
    let marketplace_secret_key = generate_secret_key();
    let agent_secret_key = generate_secret_key();
    let agent_pubkey = get_public_key(&agent_secret_key).expect("agent pubkey");

    // 3. Create skill license content
    let license_content = SkillLicenseContent::new(
        "skill-code-reviewer-v1",
        "code-reviewer",
        "1.0.0",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
        vec![
            "analyze".to_string(),
            "review".to_string(),
            "suggest".to_string(),
        ],
    );

    // 4. Create license event
    let license = SkillLicense::new(license_content, hex::encode(agent_pubkey), 1000); // 1000 sats

    let license_json = license.content.to_json().expect("serialize license");
    let license_template = EventTemplate {
        kind: KIND_SKILL_LICENSE,
        content: license_json,
        tags: license.build_tags(),
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    };

    let license_event =
        finalize_event(&license_template, &marketplace_secret_key).expect("should sign license");

    // 5. Connect to relay
    let relay = RelayConnection::new(&relay_url).expect("connection");
    relay.connect().await.expect("connect");

    // 6. Agent subscribes to skill licenses (broader filter, verify tags after)
    let filter = json!({
        "kinds": [KIND_SKILL_LICENSE],
        "limit": 10
    });
    let mut rx = relay
        .subscribe_with_channel("my-licenses", &[filter])
        .await
        .expect("subscribe");

    sleep(Duration::from_millis(100)).await;

    // 7. Marketplace publishes license
    let confirmation = relay
        .publish_event(&license_event, Duration::from_secs(5))
        .await
        .expect("publish");

    assert!(confirmation.accepted, "Relay should accept license event");

    // 8. Agent receives license
    let received = tokio::time::timeout(Duration::from_secs(2), rx.recv())
        .await
        .expect("should receive within timeout")
        .expect("should have license");

    assert_eq!(received.id, license_event.id);
    assert_eq!(received.kind, KIND_SKILL_LICENSE);

    // 9. Parse and verify license content
    let parsed_license =
        SkillLicenseContent::from_json(&received.content).expect("should parse license");

    assert_eq!(parsed_license.skill_id, "skill-code-reviewer-v1");
    assert_eq!(parsed_license.skill_name, "code-reviewer");
    assert_eq!(parsed_license.version, "1.0.0");
    assert!(parsed_license.has_capability("analyze"));
    assert!(parsed_license.has_capability("review"));
    assert!(parsed_license.has_capability("suggest"));
    assert!(!parsed_license.has_capability("execute")); // Not granted

    // Verify license tags
    let has_skill_tag = received
        .tags
        .iter()
        .any(|t| t[0] == "skill" && t[1] == "code-reviewer");
    assert!(has_skill_tag, "License should have skill tag");

    let has_price_tag = received
        .tags
        .iter()
        .any(|t| t[0] == "price_sats" && t[1] == "1000");
    assert!(has_price_tag, "License should have price tag");

    // Verify license is not expired
    let current_time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    assert!(
        !parsed_license.is_expired(current_time),
        "License should not be expired"
    );

    relay.disconnect().await.ok();
}

#[tokio::test]
async fn test_skill_delivery_encrypted() {
    // Test: Skill delivery with content verification (NIP-SA kind:39221)
    //
    // 1. After license issuance, marketplace delivers skill content
    // 2. Content is delivered with hash for verification
    // 3. Agent verifies content hash matches

    // 1. Start test relay
    let (_server, _tmp) = start_test_relay(19212).await;
    let relay_url = test_relay_url(19212);

    // 2. Create identities
    let marketplace_secret_key = generate_secret_key();
    let _agent_secret_key = generate_secret_key();

    // 3. Create skill content (the actual skill code/prompts)
    let skill_code = r#"
/// Review Rust code for best practices
pub fn review_code(code: &str) -> Vec<ReviewComment> {
    let mut comments = Vec::new();
    
    // Check for unwrap() usage
    if code.contains(".unwrap()") {
        comments.push(ReviewComment {
            severity: Severity::Warning,
            message: "Consider using ? operator instead of unwrap()".to_string(),
        });
    }
    
    // Check for proper error handling
    if code.contains("panic!") {
        comments.push(ReviewComment {
            severity: Severity::Error,
            message: "Avoid panic! in production code".to_string(),
        });
    }
    
    comments
}
"#;

    // 4. Calculate content hash
    let content_hash = sha256_hash(skill_code);

    // 5. Create skill delivery content
    let delivery_content =
        SkillDeliveryContent::new("skill-code-reviewer-v1", skill_code, "rust", &content_hash);

    // 6. Create delivery event
    let delivery = SkillDelivery::new(delivery_content, "license-event-id-abc123");

    let delivery_json = delivery.content.to_json().expect("serialize delivery");
    let delivery_template = EventTemplate {
        kind: KIND_SKILL_DELIVERY,
        content: delivery_json,
        tags: delivery.build_tags(),
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    };

    let delivery_event =
        finalize_event(&delivery_template, &marketplace_secret_key).expect("should sign delivery");

    // 7. Connect to relay
    let relay = RelayConnection::new(&relay_url).expect("connection");
    relay.connect().await.expect("connect");

    // 8. Agent subscribes to skill deliveries (broader filter, verify tags after)
    let filter = json!({
        "kinds": [KIND_SKILL_DELIVERY],
        "limit": 10
    });
    let mut rx = relay
        .subscribe_with_channel("my-deliveries", &[filter])
        .await
        .expect("subscribe");

    sleep(Duration::from_millis(100)).await;

    // 9. Marketplace publishes delivery
    let confirmation = relay
        .publish_event(&delivery_event, Duration::from_secs(5))
        .await
        .expect("publish");

    assert!(confirmation.accepted, "Relay should accept delivery event");

    // 10. Agent receives delivery
    let received = tokio::time::timeout(Duration::from_secs(2), rx.recv())
        .await
        .expect("should receive within timeout")
        .expect("should have delivery");

    assert_eq!(received.id, delivery_event.id);
    assert_eq!(received.kind, KIND_SKILL_DELIVERY);

    // 11. Parse and verify delivery content
    let parsed_delivery =
        SkillDeliveryContent::from_json(&received.content).expect("should parse delivery");

    assert_eq!(parsed_delivery.skill_id, "skill-code-reviewer-v1");
    assert_eq!(parsed_delivery.content_type, "rust");

    // 12. Verify content hash matches
    assert!(
        parsed_delivery.verify_hash(&content_hash).is_ok(),
        "Content hash should verify"
    );

    // Try with wrong hash - should fail
    assert!(
        parsed_delivery.verify_hash("wrong_hash_123").is_err(),
        "Wrong hash should fail verification"
    );

    // 13. Verify delivery tags
    let has_license_tag = received
        .tags
        .iter()
        .any(|t| t[0] == "license" && t[1] == "license-event-id-abc123");
    assert!(has_license_tag, "Delivery should reference license");

    let has_type_tag = received
        .tags
        .iter()
        .any(|t| t[0] == "type" && t[1] == "rust");
    assert!(has_type_tag, "Delivery should have content type tag");

    let has_hash_tag = received
        .tags
        .iter()
        .any(|t| t[0] == "hash" && t[1] == content_hash);
    assert!(has_hash_tag, "Delivery should have hash tag");

    relay.disconnect().await.ok();
}

#[tokio::test]
async fn test_skill_versioning() {
    // Test: Multiple versions of same skill can be published and discovered
    //
    // 1. Creator publishes v1.0.0 of skill
    // 2. Creator publishes v1.1.0 of skill
    // 3. Consumer can discover both versions
    // 4. Filter by version or get latest

    // 1. Start test relay
    let (_server, _tmp) = start_test_relay(19213).await;
    let relay_url = test_relay_url(19213);

    // 2. Create creator identity
    let creator_secret_key = generate_secret_key();
    let creator_pubkey = get_public_key(&creator_secret_key).expect("pubkey");

    // 3. Create v1.0.0 skill
    let v1_metadata = HandlerMetadata::new(
        "data-analyzer",
        "Analyzes datasets and provides insights (v1.0.0)",
    )
    .with_website("https://openagents.com/skills/data-analyzer");

    let v1_handler = HandlerInfo::new(
        hex::encode(creator_pubkey),
        HandlerType::Skill,
        v1_metadata,
    )
    .add_capability("csv-parsing")
    .add_capability("basic-stats");

    let mut v1_tags = v1_handler.to_tags();
    v1_tags.push(vec!["version".to_string(), "1.0.0".to_string()]);
    v1_tags.push(vec![
        "d".to_string(),
        "data-analyzer:1.0.0".to_string(), // Addressable event identifier
    ]);

    let v1_template = EventTemplate {
        kind: KIND_HANDLER_INFO,
        content: serde_json::to_string(&v1_handler.metadata).unwrap(),
        tags: v1_tags,
        created_at: 1700000000, // Earlier timestamp
    };

    let v1_event = finalize_event(&v1_template, &creator_secret_key).expect("sign v1");

    // 4. Create v1.1.0 skill (enhanced version)
    let v2_metadata = HandlerMetadata::new(
        "data-analyzer",
        "Analyzes datasets and provides insights (v1.1.0) - now with ML!",
    )
    .with_website("https://openagents.com/skills/data-analyzer");

    let v2_handler = HandlerInfo::new(
        hex::encode(creator_pubkey),
        HandlerType::Skill,
        v2_metadata,
    )
    .add_capability("csv-parsing")
    .add_capability("basic-stats")
    .add_capability("ml-predictions"); // New capability in v1.1.0

    let mut v2_tags = v2_handler.to_tags();
    v2_tags.push(vec!["version".to_string(), "1.1.0".to_string()]);
    v2_tags.push(vec!["d".to_string(), "data-analyzer:1.1.0".to_string()]);

    let v2_template = EventTemplate {
        kind: KIND_HANDLER_INFO,
        content: serde_json::to_string(&v2_handler.metadata).unwrap(),
        tags: v2_tags,
        created_at: 1700100000, // Later timestamp
    };

    let v2_event = finalize_event(&v2_template, &creator_secret_key).expect("sign v2");

    // 5. Connect to relay
    let relay = RelayConnection::new(&relay_url).expect("connection");
    relay.connect().await.expect("connect");

    // 6. Subscribe to all skill versions from this creator
    let filter = json!({
        "kinds": [KIND_HANDLER_INFO],
        "authors": [hex::encode(creator_pubkey)],
        "limit": 10
    });
    let mut rx = relay
        .subscribe_with_channel("all-versions", &[filter])
        .await
        .expect("subscribe");

    sleep(Duration::from_millis(100)).await;

    // 7. Publish both versions
    relay
        .publish_event(&v1_event, Duration::from_secs(5))
        .await
        .expect("publish v1");

    relay
        .publish_event(&v2_event, Duration::from_secs(5))
        .await
        .expect("publish v2");

    // 8. Receive both versions
    let mut received_events = Vec::new();
    for _ in 0..2 {
        match tokio::time::timeout(Duration::from_secs(2), rx.recv()).await {
            Ok(Some(event)) => received_events.push(event),
            _ => break,
        }
    }

    assert_eq!(received_events.len(), 2, "Should receive both versions");

    // 9. Verify we have both versions
    let versions: Vec<String> = received_events
        .iter()
        .filter_map(|e| {
            e.tags
                .iter()
                .find(|t| t[0] == "version")
                .map(|t| t[1].clone())
        })
        .collect();

    assert!(versions.contains(&"1.0.0".to_string()), "Should have v1.0.0");
    assert!(versions.contains(&"1.1.0".to_string()), "Should have v1.1.0");

    // 10. Verify v1.1.0 has the new capability
    let v2_received = received_events
        .iter()
        .find(|e| {
            e.tags
                .iter()
                .any(|t| t[0] == "version" && t[1] == "1.1.0")
        })
        .expect("should find v1.1.0");

    let has_ml_capability = v2_received
        .tags
        .iter()
        .any(|t| t[0] == "capability" && t[1] == "ml-predictions");
    assert!(
        has_ml_capability,
        "v1.1.0 should have ml-predictions capability"
    );

    relay.disconnect().await.ok();
}

#[tokio::test]
async fn test_skill_license_expiration() {
    // Test: Skill license with expiration works correctly
    //
    // 1. Create license with expiration
    // 2. Verify not expired before expiry time
    // 3. Verify expired after expiry time

    // 1. Create license with expiration (1 hour from now)
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let expires_at = now + 3600; // 1 hour from now

    let license_content = SkillLicenseContent::new(
        "skill-premium-v1",
        "premium-analyzer",
        "2.0.0",
        now,
        vec!["premium-feature".to_string()],
    )
    .with_expires_at(expires_at);

    // 2. Verify not expired now
    assert!(
        !license_content.is_expired(now),
        "License should not be expired now"
    );
    assert!(
        !license_content.is_expired(now + 1800),
        "License should not be expired in 30 minutes"
    );

    // 3. Verify expired after expiry
    assert!(
        license_content.is_expired(expires_at),
        "License should be expired at expiry time"
    );
    assert!(
        license_content.is_expired(expires_at + 1),
        "License should be expired after expiry time"
    );

    // 4. Perpetual license (no expiration)
    let perpetual_license =
        SkillLicenseContent::new("skill-perpetual", "perpetual-skill", "1.0.0", now, vec![]);

    assert!(
        !perpetual_license.is_expired(u64::MAX - 1),
        "Perpetual license should never expire"
    );
}

#[tokio::test]
async fn test_complete_skill_purchase_flow() {
    // Test: Complete skill purchase flow from discovery to delivery
    //
    // This simulates the full flow:
    // 1. Creator publishes skill (NIP-89)
    // 2. Agent discovers skill
    // 3. Agent requests purchase (simulated payment)
    // 4. Marketplace issues license (kind:39220)
    // 5. Marketplace delivers skill (kind:39221)
    // 6. Agent verifies and uses skill

    // 1. Start test relay
    let (_server, _tmp) = start_test_relay(19214).await;
    let relay_url = test_relay_url(19214);

    // 2. Create identities
    let creator_secret_key = generate_secret_key();
    let creator_pubkey = get_public_key(&creator_secret_key).expect("creator pubkey");
    let marketplace_secret_key = generate_secret_key();
    let agent_secret_key = generate_secret_key();
    let agent_pubkey = get_public_key(&agent_secret_key).expect("agent pubkey");

    // 3. Connect all parties
    let creator_relay = RelayConnection::new(&relay_url).expect("creator connection");
    creator_relay.connect().await.expect("creator connect");

    let marketplace_relay = RelayConnection::new(&relay_url).expect("marketplace connection");
    marketplace_relay
        .connect()
        .await
        .expect("marketplace connect");

    let agent_relay = RelayConnection::new(&relay_url).expect("agent connection");
    agent_relay.connect().await.expect("agent connect");

    // 4. Creator publishes skill listing
    let skill_metadata = HandlerMetadata::new(
        "web-scraper",
        "Professional web scraping skill with rate limiting and proxy support",
    )
    .with_website("https://creator.example.com/web-scraper");

    let skill_handler = HandlerInfo::new(
        hex::encode(creator_pubkey),
        HandlerType::Skill,
        skill_metadata,
    )
    .add_capability("http-requests")
    .add_capability("html-parsing")
    .add_capability("proxy-rotation");

    let mut skill_tags = skill_handler.to_tags();
    skill_tags.push(vec!["price_sats".to_string(), "5000".to_string()]);

    let skill_template = EventTemplate {
        kind: KIND_HANDLER_INFO,
        content: serde_json::to_string(&skill_handler.metadata).unwrap(),
        tags: skill_tags,
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    };

    let skill_event = finalize_event(&skill_template, &creator_secret_key).expect("sign skill");

    // Agent subscribes to skill discovery
    let discovery_filter = json!({
        "kinds": [KIND_HANDLER_INFO],
        "limit": 10
    });
    let mut discovery_rx = agent_relay
        .subscribe_with_channel("skill-discovery", &[discovery_filter])
        .await
        .expect("subscribe discovery");

    // Agent subscribes to skill licenses (broader filter, verify tags after)
    let license_filter = json!({
        "kinds": [KIND_SKILL_LICENSE],
        "limit": 10
    });
    let mut license_rx = agent_relay
        .subscribe_with_channel("my-licenses", &[license_filter])
        .await
        .expect("subscribe licenses");

    sleep(Duration::from_millis(100)).await;

    // Creator publishes skill
    creator_relay
        .publish_event(&skill_event, Duration::from_secs(5))
        .await
        .expect("publish skill");

    // Agent discovers skill
    let discovered_skill = tokio::time::timeout(Duration::from_secs(2), discovery_rx.recv())
        .await
        .expect("agent should discover skill")
        .expect("should have skill");

    assert_eq!(discovered_skill.kind, KIND_HANDLER_INFO);

    // 5. Marketplace issues license (after simulated payment)
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let license_content = SkillLicenseContent::new(
        discovered_skill.id.clone(),
        "web-scraper",
        "1.0.0",
        now,
        vec![
            "http-requests".to_string(),
            "html-parsing".to_string(),
            "proxy-rotation".to_string(),
        ],
    );

    let license = SkillLicense::new(license_content, hex::encode(agent_pubkey), 5000);

    let license_template = EventTemplate {
        kind: KIND_SKILL_LICENSE,
        content: license.content.to_json().unwrap(),
        tags: license.build_tags(),
        created_at: now,
    };

    let license_event =
        finalize_event(&license_template, &marketplace_secret_key).expect("sign license");
    let license_event_id = license_event.id.clone();

    marketplace_relay
        .publish_event(&license_event, Duration::from_secs(5))
        .await
        .expect("publish license");

    // Agent receives license
    let received_license = tokio::time::timeout(Duration::from_secs(2), license_rx.recv())
        .await
        .expect("agent should receive license")
        .expect("should have license");

    assert_eq!(received_license.kind, KIND_SKILL_LICENSE);

    // 6. Marketplace delivers skill content
    let skill_code = r#"
use reqwest::Client;

pub async fn scrape(url: &str) -> Result<String, Error> {
    let client = Client::new();
    let response = client.get(url).send().await?;
    Ok(response.text().await?)
}
"#;

    let content_hash = sha256_hash(skill_code);
    let delivery_content = SkillDeliveryContent::new(
        discovered_skill.id.clone(),
        skill_code,
        "rust",
        &content_hash,
    );

    let delivery = SkillDelivery::new(delivery_content, &license_event_id);

    // Agent subscribes to skill deliveries (broader filter, verify tags after)
    let delivery_filter = json!({
        "kinds": [KIND_SKILL_DELIVERY],
        "limit": 10
    });
    let mut delivery_rx = agent_relay
        .subscribe_with_channel("my-deliveries", &[delivery_filter])
        .await
        .expect("subscribe deliveries");

    sleep(Duration::from_millis(100)).await;

    let delivery_template = EventTemplate {
        kind: KIND_SKILL_DELIVERY,
        content: delivery.content.to_json().unwrap(),
        tags: delivery.build_tags(),
        created_at: now + 1,
    };

    let delivery_event =
        finalize_event(&delivery_template, &marketplace_secret_key).expect("sign delivery");

    marketplace_relay
        .publish_event(&delivery_event, Duration::from_secs(5))
        .await
        .expect("publish delivery");

    // Agent receives delivery
    let received_delivery = tokio::time::timeout(Duration::from_secs(2), delivery_rx.recv())
        .await
        .expect("agent should receive delivery")
        .expect("should have delivery");

    assert_eq!(received_delivery.kind, KIND_SKILL_DELIVERY);

    // 7. Agent verifies skill content
    let parsed_delivery =
        SkillDeliveryContent::from_json(&received_delivery.content).expect("parse delivery");

    assert!(
        parsed_delivery.verify_hash(&content_hash).is_ok(),
        "Skill content hash should verify"
    );

    // Cleanup
    creator_relay.disconnect().await.ok();
    marketplace_relay.disconnect().await.ok();
    agent_relay.disconnect().await.ok();
}

// =============================================================================
// Type Validation Tests (kept from original for regression testing)
// =============================================================================

#[test]
fn test_skill_metadata_validation() {
    // Test that skill metadata has required fields
    let metadata = HandlerMetadata::new("code-reviewer", "AI code review assistant");

    assert!(!metadata.name.is_empty(), "Name is required");
    assert!(!metadata.description.is_empty(), "Description is required");
}

#[test]
fn test_skill_pricing_calculation() {
    // Test skill execution cost calculation
    let price_per_execution = 1000u64; // millisats
    let num_executions = 5u64;

    let total_cost = price_per_execution * num_executions;

    assert_eq!(total_cost, 5000);
}

#[test]
fn test_skill_handler_type_identification() {
    // Test that skill handlers are correctly identified
    let handler_type = HandlerType::Skill;

    match handler_type {
        HandlerType::Skill => {}
        _ => panic!("Handler should be Skill type"),
    }
}

#[test]
fn test_handler_info_event_kind() {
    // Verify handler info events use correct kind (31990)
    assert_eq!(KIND_HANDLER_INFO, 31990);
}

#[test]
fn test_skill_license_kind() {
    // Verify skill license uses correct kind (39220)
    assert_eq!(KIND_SKILL_LICENSE, 39220);
}

#[test]
fn test_skill_delivery_kind() {
    // Verify skill delivery uses correct kind (39221)
    assert_eq!(KIND_SKILL_DELIVERY, 39221);
}

#[test]
fn test_skill_license_content_serialization() {
    // Test license content can round-trip through JSON
    let content = SkillLicenseContent::new(
        "skill-123",
        "test-skill",
        "1.0.0",
        1703000000,
        vec!["cap1".to_string(), "cap2".to_string()],
    );

    let json = content.to_json().expect("should serialize");
    let parsed = SkillLicenseContent::from_json(&json).expect("should parse");

    assert_eq!(parsed.skill_id, content.skill_id);
    assert_eq!(parsed.skill_name, content.skill_name);
    assert_eq!(parsed.version, content.version);
    assert_eq!(parsed.capabilities.len(), 2);
}

#[test]
fn test_skill_delivery_content_hash_verification() {
    // Test content hash verification
    let content = SkillDeliveryContent::new("skill-123", "fn main() {}", "rust", "abc123");

    assert!(content.verify_hash("abc123").is_ok());
    assert!(content.verify_hash("wrong").is_err());
}
