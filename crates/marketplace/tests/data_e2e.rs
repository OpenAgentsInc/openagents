//! End-to-end integration tests for Data Marketplace over real Nostr relays
//!
//! Tests data listing discovery, publish, purchase, and delivery flows.
//! Part of d-015: Comprehensive Marketplace and Agent Commerce E2E Tests

use chrono::Utc;
use marketplace::core::payments::MockPaymentService;
use marketplace::{DataListing, DataListingType, DatasetMetadata};
use nostr::{EventTemplate, finalize_event, generate_secret_key, get_public_key};
use nostr::{HandlerMetadata, KIND_HANDLER_INFO};
use nostr_client::RelayConnection;
use nostr_relay::{Database, DatabaseConfig, RelayConfig, RelayServer};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

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

    sleep(Duration::from_millis(200)).await;
    (server, temp_dir)
}

fn test_relay_url(port: u16) -> String {
    format!("ws://127.0.0.1:{}", port)
}

fn sha256_hash(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    hex::encode(hasher.finalize())
}

// =============================================================================
// Phase 3 Tests: Data Marketplace E2E with Real Relay
// =============================================================================

#[tokio::test]
async fn test_dataset_discovery_over_relay() {
    // Test: Discover data listings published as NIP-89 handler info events
    //
    // 1. Data provider publishes dataset listing
    // 2. Consumer discovers listings via relay query
    // 3. Verify listing metadata matches

    let (_server, _tmp) = start_test_relay(19220).await;
    let relay_url = test_relay_url(19220);

    let provider_secret_key = generate_secret_key();
    let provider_pubkey = get_public_key(&provider_secret_key).expect("pubkey");

    // Create data listing metadata
    let data_metadata = HandlerMetadata::new(
        "rust-coding-sessions-2025",
        "10,000 anonymized Rust coding sessions from professional developers",
    )
    .with_website("https://data.openagents.com/rust-sessions");

    let tags = vec![
        vec!["d".to_string(), "rust-coding-sessions-2025".to_string()],
        vec!["p".to_string(), hex::encode(provider_pubkey)],
        vec!["handler".to_string(), "data".to_string()],
        vec!["capability".to_string(), "training-data".to_string()],
        vec!["capability".to_string(), "rust-sessions".to_string()],
        vec!["price_sats".to_string(), "50000".to_string()],
        vec!["record_count".to_string(), "10000".to_string()],
        vec!["size_mb".to_string(), "500".to_string()],
    ];

    let data_template = EventTemplate {
        kind: KIND_HANDLER_INFO,
        content: serde_json::to_string(&data_metadata).unwrap(),
        tags,
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    };

    let data_event = finalize_event(&data_template, &provider_secret_key).expect("sign");

    let relay = RelayConnection::new(&relay_url).expect("connection");
    relay.connect().await.expect("connect");

    let filter = json!({
        "kinds": [KIND_HANDLER_INFO],
        "limit": 10
    });
    let mut rx = relay
        .subscribe_with_channel("data-discovery", &[filter])
        .await
        .expect("subscribe");

    sleep(Duration::from_millis(100)).await;

    let confirmation = relay
        .publish_event(&data_event, Duration::from_secs(5))
        .await
        .expect("publish");

    assert!(confirmation.accepted);

    let received = tokio::time::timeout(Duration::from_secs(2), rx.recv())
        .await
        .expect("should receive")
        .expect("should have data listing");

    assert_eq!(received.id, data_event.id);
    assert_eq!(received.kind, KIND_HANDLER_INFO);

    let parsed: HandlerMetadata = serde_json::from_str(&received.content).expect("should parse");
    assert_eq!(parsed.name, "rust-coding-sessions-2025");

    let has_data_type = received
        .tags
        .iter()
        .any(|t| t[0] == "handler" && t[1] == "data");
    assert!(has_data_type, "Should have data handler type");

    let has_price = received
        .tags
        .iter()
        .any(|t| t[0] == "price_sats" && t[1] == "50000");
    assert!(has_price, "Should have price tag");

    relay.disconnect().await.ok();
}

#[tokio::test]
async fn test_dataset_publish_flow() {
    // Test: Complete dataset publish flow with full metadata
    //
    // 1. Provider creates detailed dataset listing
    // 2. Publishes to relay with all metadata
    // 3. Verify all metadata fields are preserved

    let (_server, _tmp) = start_test_relay(19221).await;
    let relay_url = test_relay_url(19221);

    let provider_secret_key = generate_secret_key();
    let provider_pubkey = get_public_key(&provider_secret_key).expect("pubkey");

    // Create DataListing with full metadata
    let now = Utc::now();
    let start_date = now - chrono::Duration::days(90);
    let metadata = DatasetMetadata::new(
        vec!["codex".to_string(), "cursor".to_string()],
        vec![
            "rust".to_string(),
            "python".to_string(),
            "typescript".to_string(),
        ],
        (start_date, now),
        0.92,
    )
    .with_contributor_count(500)
    .with_avg_session_duration(1800);

    let listing = DataListing::new(
        "dataset-premium-2025-01",
        DataListingType::Premium,
        "Premium curated coding sessions with expert annotations",
        75000,
        1024 * 1024 * 750,
        5000,
        metadata,
    );

    // Convert to handler info for relay
    let tags = vec![
        vec!["d".to_string(), listing.id.clone()],
        vec!["p".to_string(), hex::encode(provider_pubkey)],
        vec!["handler".to_string(), "data".to_string()],
        vec!["capability".to_string(), "premium-data".to_string()],
        vec!["capability".to_string(), "annotated".to_string()],
        vec!["price_sats".to_string(), listing.price_sats.to_string()],
        vec!["record_count".to_string(), listing.record_count.to_string()],
        vec!["size_bytes".to_string(), listing.size_bytes.to_string()],
        vec!["listing_type".to_string(), "premium".to_string()],
        vec!["quality_score".to_string(), "0.92".to_string()],
    ];

    let content = serde_json::to_string(&listing).expect("serialize listing");

    let publish_template = EventTemplate {
        kind: KIND_HANDLER_INFO,
        content,
        tags,
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    };

    let publish_event = finalize_event(&publish_template, &provider_secret_key).expect("sign");

    let relay = RelayConnection::new(&relay_url).expect("connection");
    relay.connect().await.expect("connect");

    let filter = json!({
        "kinds": [KIND_HANDLER_INFO],
        "#d": [&listing.id],
        "limit": 10
    });
    let mut rx = relay
        .subscribe_with_channel("my-listings", &[filter])
        .await
        .expect("subscribe");

    sleep(Duration::from_millis(100)).await;

    let confirmation = relay
        .publish_event(&publish_event, Duration::from_secs(5))
        .await
        .expect("publish");

    assert!(confirmation.accepted);

    let received = tokio::time::timeout(Duration::from_secs(2), rx.recv())
        .await
        .expect("should receive")
        .expect("should have listing");

    // Parse and verify full listing
    let parsed_listing: DataListing =
        serde_json::from_str(&received.content).expect("should parse listing");

    assert_eq!(parsed_listing.id, "dataset-premium-2025-01");
    assert_eq!(parsed_listing.price_sats, 75000);
    assert_eq!(parsed_listing.record_count, 5000);
    assert!(matches!(
        parsed_listing.listing_type,
        DataListingType::Premium
    ));

    // Verify metadata
    assert_eq!(parsed_listing.metadata.sources.len(), 2);
    assert!(
        parsed_listing
            .metadata
            .sources
            .contains(&"codex".to_string())
    );
    assert_eq!(parsed_listing.metadata.languages.len(), 3);
    assert_eq!(parsed_listing.metadata.quality_score, 0.92);
    assert_eq!(parsed_listing.metadata.contributor_count, Some(500));

    relay.disconnect().await.ok();
}

#[tokio::test]
async fn test_dataset_purchase_mock() {
    // Test: Dataset purchase using MockPaymentService
    //
    // 1. Consumer finds dataset listing
    // 2. Consumer pays with MockPaymentService
    // 3. Provider verifies payment and grants access

    let (_server, _tmp) = start_test_relay(19222).await;
    let relay_url = test_relay_url(19222);

    let provider_secret_key = generate_secret_key();
    let provider_pubkey = get_public_key(&provider_secret_key).expect("pubkey");
    let consumer_secret_key = generate_secret_key();
    let consumer_pubkey = get_public_key(&consumer_secret_key).expect("pubkey");

    // Publish dataset listing
    let handler_metadata = HandlerMetadata::new(
        "ml-training-dataset",
        "Machine learning training dataset for code completion",
    );

    let tags = vec![
        vec!["d".to_string(), "ml-training-dataset".to_string()],
        vec!["p".to_string(), hex::encode(provider_pubkey)],
        vec!["handler".to_string(), "data".to_string()],
        vec!["price_sats".to_string(), "25000".to_string()],
    ];

    let listing_template = EventTemplate {
        kind: KIND_HANDLER_INFO,
        content: serde_json::to_string(&handler_metadata).unwrap(),
        tags,
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    };

    let listing_event = finalize_event(&listing_template, &provider_secret_key).expect("sign");

    let relay = RelayConnection::new(&relay_url).expect("connection");
    relay.connect().await.expect("connect");

    relay
        .publish_event(&listing_event, Duration::from_secs(5))
        .await
        .expect("publish listing");

    // Consumer initiates purchase with MockPaymentService
    let mut payment_service = MockPaymentService::new();
    let mock_invoice = payment_service.create_invoice(25000, "Dataset purchase");

    assert!(
        !payment_service.is_paid(&mock_invoice.invoice),
        "Invoice should not be paid yet"
    );

    // Consumer pays
    let preimage = payment_service
        .pay_invoice(&mock_invoice.invoice)
        .expect("payment should succeed");
    assert!(!preimage.is_empty(), "Preimage should not be empty");
    assert!(
        payment_service.is_paid(&mock_invoice.invoice),
        "Invoice should be paid now"
    );

    // Provider creates access grant event after payment verification
    let access_grant_kind: u16 = 38040; // Custom kind for data access grant
    let grant_tags = vec![
        vec!["d".to_string(), "ml-training-dataset".to_string()],
        vec!["p".to_string(), hex::encode(consumer_pubkey)],
        vec!["invoice".to_string(), mock_invoice.invoice.clone()],
        vec!["preimage".to_string(), preimage.clone()],
    ];

    let grant_content = json!({
        "dataset_id": "ml-training-dataset",
        "consumer_pubkey": hex::encode(consumer_pubkey),
        "access_type": "full",
        "expires_at": null,
        "download_url": "https://data.openagents.com/download/ml-training-dataset",
    });

    let grant_template = EventTemplate {
        kind: access_grant_kind,
        content: grant_content.to_string(),
        tags: grant_tags,
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    };

    let grant_event = finalize_event(&grant_template, &provider_secret_key).expect("sign grant");

    // Subscribe to access grants for consumer
    let grant_filter = json!({
        "kinds": [access_grant_kind],
        "#p": [hex::encode(consumer_pubkey)],
        "limit": 10
    });
    let mut grant_rx = relay
        .subscribe_with_channel("my-grants", &[grant_filter])
        .await
        .expect("subscribe");

    sleep(Duration::from_millis(100)).await;

    relay
        .publish_event(&grant_event, Duration::from_secs(5))
        .await
        .expect("publish grant");

    let received_grant = tokio::time::timeout(Duration::from_secs(2), grant_rx.recv())
        .await
        .expect("should receive grant")
        .expect("should have grant");

    assert_eq!(received_grant.kind, access_grant_kind);

    let grant_data: serde_json::Value =
        serde_json::from_str(&received_grant.content).expect("parse grant");
    assert_eq!(grant_data["dataset_id"], "ml-training-dataset");
    assert_eq!(grant_data["access_type"], "full");

    relay.disconnect().await.ok();
}

#[tokio::test]
async fn test_dataset_encrypted_delivery() {
    // Test: Dataset delivery with content hash verification
    //
    // 1. After purchase, provider delivers data chunk
    // 2. Data includes content hash for verification
    // 3. Consumer verifies hash matches

    let (_server, _tmp) = start_test_relay(19223).await;
    let relay_url = test_relay_url(19223);

    let provider_secret_key = generate_secret_key();
    let consumer_secret_key = generate_secret_key();
    let consumer_pubkey = get_public_key(&consumer_secret_key).expect("pubkey");

    // Simulate data chunk (in reality this would be encrypted)
    let data_chunk = r#"[
        {"session_id": "sess_001", "tool_calls": 42, "tokens_in": 15000, "tokens_out": 8000},
        {"session_id": "sess_002", "tool_calls": 38, "tokens_in": 12000, "tokens_out": 6500},
        {"session_id": "sess_003", "tool_calls": 55, "tokens_in": 20000, "tokens_out": 11000}
    ]"#;

    let content_hash = sha256_hash(data_chunk);

    let delivery_kind: u16 = 38041; // Custom kind for data delivery
    let delivery_tags = vec![
        vec!["p".to_string(), hex::encode(consumer_pubkey)],
        vec!["dataset".to_string(), "ml-training-dataset".to_string()],
        vec!["chunk".to_string(), "1".to_string()],
        vec!["total_chunks".to_string(), "10".to_string()],
        vec!["hash".to_string(), content_hash.clone()],
    ];

    let delivery_content = json!({
        "chunk_index": 1,
        "data": data_chunk,
        "hash": content_hash,
    });

    let delivery_template = EventTemplate {
        kind: delivery_kind,
        content: delivery_content.to_string(),
        tags: delivery_tags,
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    };

    let delivery_event = finalize_event(&delivery_template, &provider_secret_key).expect("sign");

    let relay = RelayConnection::new(&relay_url).expect("connection");
    relay.connect().await.expect("connect");

    let filter = json!({
        "kinds": [delivery_kind],
        "#p": [hex::encode(consumer_pubkey)],
        "limit": 10
    });
    let mut rx = relay
        .subscribe_with_channel("my-data", &[filter])
        .await
        .expect("subscribe");

    sleep(Duration::from_millis(100)).await;

    relay
        .publish_event(&delivery_event, Duration::from_secs(5))
        .await
        .expect("publish");

    let received = tokio::time::timeout(Duration::from_secs(2), rx.recv())
        .await
        .expect("should receive")
        .expect("should have delivery");

    assert_eq!(received.kind, delivery_kind);

    // Verify hash
    let received_content: serde_json::Value =
        serde_json::from_str(&received.content).expect("parse");
    let received_data = received_content["data"].as_str().expect("data field");
    let received_hash = received_content["hash"].as_str().expect("hash field");

    let computed_hash = sha256_hash(received_data);
    assert_eq!(computed_hash, received_hash, "Content hash should match");
    assert_eq!(computed_hash, content_hash, "Hash should match original");

    // Verify chunk metadata
    let has_chunk_tag = received.tags.iter().any(|t| t[0] == "chunk" && t[1] == "1");
    assert!(has_chunk_tag, "Should have chunk tag");

    let has_total_tag = received
        .tags
        .iter()
        .any(|t| t[0] == "total_chunks" && t[1] == "10");
    assert!(has_total_tag, "Should have total_chunks tag");

    relay.disconnect().await.ok();
}

#[tokio::test]
async fn test_data_listing_filtering() {
    // Test: Filter data listings by type, price, etc.

    let (_server, _tmp) = start_test_relay(19224).await;
    let relay_url = test_relay_url(19224);

    let provider_secret_key = generate_secret_key();
    let provider_pubkey = get_public_key(&provider_secret_key).expect("pubkey");

    // Publish multiple listings with different types
    let listings = vec![
        ("premium-rust", "Premium Rust sessions", "premium", 100000),
        (
            "standard-python",
            "Standard Python sessions",
            "standard",
            50000,
        ),
        (
            "aggregated-stats",
            "Aggregated statistics",
            "aggregated",
            10000,
        ),
    ];

    let relay = RelayConnection::new(&relay_url).expect("connection");
    relay.connect().await.expect("connect");

    for (id, desc, listing_type, price) in &listings {
        let handler_metadata = HandlerMetadata::new(*id, *desc);

        let tags = vec![
            vec!["d".to_string(), id.to_string()],
            vec!["p".to_string(), hex::encode(provider_pubkey)],
            vec!["handler".to_string(), "data".to_string()],
            vec!["listing_type".to_string(), listing_type.to_string()],
            vec!["price_sats".to_string(), price.to_string()],
        ];

        let template = EventTemplate {
            kind: KIND_HANDLER_INFO,
            content: serde_json::to_string(&handler_metadata).unwrap(),
            tags,
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        };

        let event = finalize_event(&template, &provider_secret_key).expect("sign");
        relay
            .publish_event(&event, Duration::from_secs(5))
            .await
            .expect("publish");
    }

    // Query for all listings from this provider
    let filter = json!({
        "kinds": [KIND_HANDLER_INFO],
        "authors": [hex::encode(provider_pubkey)],
        "limit": 10
    });
    let mut rx = relay
        .subscribe_with_channel("all-listings", &[filter])
        .await
        .expect("subscribe");

    sleep(Duration::from_millis(100)).await;

    // Receive all listings
    let mut received_listings = Vec::new();
    for _ in 0..3 {
        match tokio::time::timeout(Duration::from_secs(2), rx.recv()).await {
            Ok(Some(event)) => received_listings.push(event),
            _ => break,
        }
    }

    assert_eq!(received_listings.len(), 3, "Should receive all 3 listings");

    // Verify listing types
    let types: Vec<String> = received_listings
        .iter()
        .filter_map(|e| {
            e.tags
                .iter()
                .find(|t| t[0] == "listing_type")
                .map(|t| t[1].clone())
        })
        .collect();

    assert!(types.contains(&"premium".to_string()));
    assert!(types.contains(&"standard".to_string()));
    assert!(types.contains(&"aggregated".to_string()));

    relay.disconnect().await.ok();
}

// =============================================================================
// Type Validation Tests
// =============================================================================

#[test]
fn test_data_listing_type_descriptions() {
    assert!(!DataListingType::Premium.description().is_empty());
    assert!(!DataListingType::Standard.description().is_empty());
    assert!(!DataListingType::Aggregated.description().is_empty());
}

#[test]
fn test_data_listing_price_ranges() {
    let (min, max) = DataListingType::Premium.typical_price_range();
    assert!(max > min, "Max should be greater than min");
    assert!(min >= 10000, "Premium minimum should be at least 10k sats");

    let (std_min, std_max) = DataListingType::Standard.typical_price_range();
    assert!(std_max > std_min);
}

#[test]
fn test_dataset_metadata_creation() {
    let now = Utc::now();
    let start = now - chrono::Duration::days(30);

    let metadata = DatasetMetadata::new(
        vec!["source1".to_string()],
        vec!["rust".to_string()],
        (start, now),
        0.85,
    )
    .with_contributor_count(100)
    .with_avg_session_duration(3600);

    assert_eq!(metadata.sources.len(), 1);
    assert_eq!(metadata.languages.len(), 1);
    assert_eq!(metadata.quality_score, 0.85);
    assert_eq!(metadata.contributor_count, Some(100));
    assert_eq!(metadata.avg_session_duration, Some(3600));
}

#[test]
fn test_data_listing_serialization() {
    let now = Utc::now();
    let metadata = DatasetMetadata::new(
        vec!["codex".to_string()],
        vec!["rust".to_string()],
        (now - chrono::Duration::days(7), now),
        0.9,
    );

    let listing = DataListing::new(
        "test-listing",
        DataListingType::Standard,
        "Test listing",
        50000,
        1024 * 1024,
        100,
        metadata,
    );

    let json = serde_json::to_string(&listing).expect("should serialize");
    let parsed: DataListing = serde_json::from_str(&json).expect("should deserialize");

    assert_eq!(parsed.id, "test-listing");
    assert_eq!(parsed.price_sats, 50000);
    assert_eq!(parsed.record_count, 100);
}

#[test]
fn test_mock_payment_service() {
    let mut service = MockPaymentService::new();

    let invoice1 = service.create_invoice(1000, "Test payment 1");
    let invoice2 = service.create_invoice(2000, "Test payment 2");

    assert!(!service.is_paid(&invoice1.invoice));
    assert!(!service.is_paid(&invoice2.invoice));

    let preimage = service.pay_invoice(&invoice1.invoice).expect("should pay");
    assert!(!preimage.is_empty());
    assert!(service.is_paid(&invoice1.invoice));
    assert!(!service.is_paid(&invoice2.invoice));

    // Paying same invoice again should fail
    assert!(service.pay_invoice(&invoice1.invoice).is_err());

    // Paying unknown invoice should fail
    assert!(service.pay_invoice("unknown").is_err());
}
