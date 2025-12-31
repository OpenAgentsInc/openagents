//! End-to-end integration tests for NIP-90 compute marketplace over real Nostr relays
//!
//! These tests verify that the complete compute marketplace stack works correctly
//! over real relay connections, testing:
//! - NIP-90 job request publishing and fetching
//! - Job result lifecycle (pending → running → completed)
//! - Job feedback flow
//! - NIP-89 provider discovery
//! - DVM service integration with relay
//!
//! Part of d-015: Comprehensive Marketplace and Agent Commerce E2E Tests

use nostr::{EventTemplate, finalize_event, generate_secret_key, get_public_key};
use nostr::{HandlerInfo, HandlerMetadata, HandlerType, KIND_HANDLER_INFO};
use nostr::{
    JobFeedback, JobInput, JobRequest, JobResult, JobStatus, KIND_JOB_FEEDBACK,
    KIND_JOB_TEXT_GENERATION,
};
use nostr_client::RelayConnection;
use nostr_relay::{Database, DatabaseConfig, RelayConfig, RelayServer};
use serde_json::json;
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

// =============================================================================
// Phase 1 Tests: NIP-90 Compute E2E with Real Relay
// =============================================================================

#[tokio::test]
async fn test_nip90_job_request_publish_fetch() {
    // 1. Start test relay
    let (_server, _tmp) = start_test_relay(19200).await;
    let relay_url = test_relay_url(19200);

    // 2. Create customer identity
    let customer_secret_key = generate_secret_key();
    let customer_pubkey = get_public_key(&customer_secret_key).expect("pubkey");

    // 3. Create a NIP-90 job request for text generation
    let job_request = JobRequest::new(KIND_JOB_TEXT_GENERATION)
        .expect("should create job request")
        .add_input(JobInput::text("Write a haiku about decentralized AI"))
        .add_param("model", "llama3.2")
        .add_param("temperature", "0.7")
        .with_bid(1000);

    // 4. Verify job request structure
    assert_eq!(job_request.kind, KIND_JOB_TEXT_GENERATION);
    assert_eq!(job_request.inputs.len(), 1);
    assert_eq!(job_request.params.len(), 2);
    assert_eq!(job_request.bid, Some(1000));

    // 5. Convert job request to Nostr event
    let template = EventTemplate {
        kind: job_request.kind,
        content: job_request.content.clone(),
        tags: job_request.to_tags(),
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    };

    let event = finalize_event(&template, &customer_secret_key).expect("should sign event");

    // 6. Connect to relay and subscribe BEFORE publishing
    let relay = RelayConnection::new(&relay_url).expect("connection");
    relay.connect().await.expect("connect");

    // Subscribe to job requests (provider perspective)
    let filter = json!({
        "kinds": [KIND_JOB_TEXT_GENERATION],
        "limit": 10
    });
    let mut rx = relay
        .subscribe_with_channel("job-requests", &[filter])
        .await
        .expect("subscribe");

    // Small delay for subscription to be active
    sleep(Duration::from_millis(100)).await;

    // 7. Publish event to relay
    let confirmation = relay
        .publish_event(&event, Duration::from_secs(5))
        .await
        .expect("publish");

    assert!(confirmation.accepted, "Relay should accept event");
    assert_eq!(confirmation.event_id, event.id);

    // 8. Receive the event on our subscription
    let received = tokio::time::timeout(Duration::from_secs(2), rx.recv())
        .await
        .expect("should receive within timeout")
        .expect("should have event");

    // 9. Verify received event matches published
    assert_eq!(received.id, event.id);
    assert_eq!(received.kind, KIND_JOB_TEXT_GENERATION);
    assert_eq!(received.pubkey, hex::encode(customer_pubkey));
    assert_eq!(received.tags.len(), 4); // 1 input + 2 params + 1 bid

    relay.disconnect().await.ok();
}

#[tokio::test]
async fn test_nip90_job_result_lifecycle() {
    // 1. Start test relay
    let (_server, _tmp) = start_test_relay(19201).await;
    let relay_url = test_relay_url(19201);

    // 2. Create identities
    let provider_secret_key = generate_secret_key();
    let customer_secret_key = generate_secret_key();
    let customer_pubkey = get_public_key(&customer_secret_key).expect("pubkey");

    // 3. Create job request event
    let job_request = JobRequest::new(KIND_JOB_TEXT_GENERATION)
        .expect("should create job request")
        .add_input(JobInput::text("Write a haiku"))
        .with_bid(500);

    let request_template = EventTemplate {
        kind: job_request.kind,
        content: job_request.content.clone(),
        tags: job_request.to_tags(),
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    };

    let request_event =
        finalize_event(&request_template, &customer_secret_key).expect("should sign request");

    // 4. Connect to relay
    let relay = RelayConnection::new(&relay_url).expect("connection");
    relay.connect().await.expect("connect");

    // 5. Subscribe to job results (customer perspective)
    let result_kind = KIND_JOB_TEXT_GENERATION + 1000; // 6050
    let filter = json!({
        "kinds": [result_kind],
        "#p": [hex::encode(customer_pubkey)],
        "limit": 10
    });
    let mut rx = relay
        .subscribe_with_channel("job-results", &[filter])
        .await
        .expect("subscribe");

    sleep(Duration::from_millis(100)).await;

    // 6. Publish the job request
    relay
        .publish_event(&request_event, Duration::from_secs(5))
        .await
        .expect("publish request");

    // 7. Provider creates and publishes job result
    let result_content = "Code flows like streams\nDecentralized and open\nAI helps us build";

    let job_result = JobResult::new(
        KIND_JOB_TEXT_GENERATION,
        request_event.id.clone(),
        hex::encode(customer_pubkey),
        result_content.to_string(),
    )
    .expect("should create job result");

    let result_template = EventTemplate {
        kind: job_result.kind,
        content: job_result.content.clone(),
        tags: job_result.to_tags(),
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    };

    let result_event =
        finalize_event(&result_template, &provider_secret_key).expect("should sign result");

    // 8. Publish result
    let confirmation = relay
        .publish_event(&result_event, Duration::from_secs(5))
        .await
        .expect("publish result");

    assert!(confirmation.accepted);

    // 9. Customer receives result
    let received = tokio::time::timeout(Duration::from_secs(2), rx.recv())
        .await
        .expect("should receive within timeout")
        .expect("should have result");

    assert_eq!(received.id, result_event.id);
    assert_eq!(received.kind, result_kind);
    assert_eq!(received.content, result_content);

    // Verify result references original request
    let has_request_ref = received
        .tags
        .iter()
        .any(|t| t[0] == "e" && t[1] == request_event.id);
    assert!(has_request_ref, "Result should reference request event");

    relay.disconnect().await.ok();
}

#[tokio::test]
async fn test_nip90_job_feedback_flow() {
    // 1. Start test relay
    let (_server, _tmp) = start_test_relay(19202).await;
    let relay_url = test_relay_url(19202);

    // 2. Create identities
    let provider_secret_key = generate_secret_key();
    let customer_secret_key = generate_secret_key();
    let customer_pubkey = get_public_key(&customer_secret_key).expect("pubkey");

    // 3. Create job request
    let job_request = JobRequest::new(KIND_JOB_TEXT_GENERATION)
        .expect("should create job request")
        .add_input(JobInput::text("Analyze the bitcoin whitepaper"))
        .with_bid(2000);

    let request_template = EventTemplate {
        kind: job_request.kind,
        content: job_request.content.clone(),
        tags: job_request.to_tags(),
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    };

    let request_event =
        finalize_event(&request_template, &customer_secret_key).expect("should sign request");

    // 4. Connect to relay
    let relay = RelayConnection::new(&relay_url).expect("connection");
    relay.connect().await.expect("connect");

    // 5. Subscribe to feedback events (customer perspective)
    let filter = json!({
        "kinds": [KIND_JOB_FEEDBACK],
        "#p": [hex::encode(customer_pubkey)],
        "limit": 10
    });
    let mut rx = relay
        .subscribe_with_channel("job-feedback", &[filter])
        .await
        .expect("subscribe");

    sleep(Duration::from_millis(100)).await;

    // 6. Publish job request
    relay
        .publish_event(&request_event, Duration::from_secs(5))
        .await
        .expect("publish request");

    // 7. Provider sends "processing" feedback
    let processing_feedback = JobFeedback::new(
        JobStatus::Processing,
        request_event.id.clone(),
        hex::encode(customer_pubkey),
    )
    .with_status_extra("Starting analysis, ETA 2 minutes");

    let feedback_template = EventTemplate {
        kind: KIND_JOB_FEEDBACK,
        content: processing_feedback.content.clone(),
        tags: processing_feedback.to_tags(),
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    };

    let feedback_event =
        finalize_event(&feedback_template, &provider_secret_key).expect("should sign feedback");

    // 8. Publish feedback
    let confirmation = relay
        .publish_event(&feedback_event, Duration::from_secs(5))
        .await
        .expect("publish feedback");

    assert!(confirmation.accepted);

    // 9. Customer receives feedback
    let received = tokio::time::timeout(Duration::from_secs(2), rx.recv())
        .await
        .expect("should receive within timeout")
        .expect("should have feedback");

    assert_eq!(received.id, feedback_event.id);
    assert_eq!(received.kind, KIND_JOB_FEEDBACK);

    // Verify status tag
    let has_status = received
        .tags
        .iter()
        .any(|t| t[0] == "status" && t[1] == "processing");
    assert!(has_status, "Feedback should have processing status");

    // 10. Provider sends "success" feedback
    let success_feedback = JobFeedback::new(
        JobStatus::Success,
        request_event.id.clone(),
        hex::encode(customer_pubkey),
    )
    .with_status_extra("Analysis complete");

    let success_template = EventTemplate {
        kind: KIND_JOB_FEEDBACK,
        content: success_feedback.content.clone(),
        tags: success_feedback.to_tags(),
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
            + 1, // Slightly later timestamp
    };

    let success_event =
        finalize_event(&success_template, &provider_secret_key).expect("should sign success");

    relay
        .publish_event(&success_event, Duration::from_secs(5))
        .await
        .expect("publish success");

    let received_success = tokio::time::timeout(Duration::from_secs(2), rx.recv())
        .await
        .expect("should receive success")
        .expect("should have success feedback");

    let has_success_status = received_success
        .tags
        .iter()
        .any(|t| t[0] == "status" && t[1] == "success");
    assert!(has_success_status, "Feedback should have success status");

    relay.disconnect().await.ok();
}

#[tokio::test]
async fn test_nip89_provider_discovery() {
    // 1. Start test relay
    let (_server, _tmp) = start_test_relay(19203).await;
    let relay_url = test_relay_url(19203);

    // 2. Create provider identity
    let provider_secret_key = generate_secret_key();
    let provider_pubkey = get_public_key(&provider_secret_key).expect("pubkey");

    // 3. Create handler metadata (NIP-89 announcement)
    let metadata = HandlerMetadata::new(
        "OpenAgents Text Generator",
        "High-quality text generation using local AI models",
    )
    .with_icon("https://openagents.com/logo.png")
    .with_website("https://openagents.com");

    // 4. Create handler info for a compute provider
    let handler_info = HandlerInfo::new(
        hex::encode(provider_pubkey),
        HandlerType::ComputeProvider,
        metadata.clone(),
    )
    .add_capability("text-generation")
    .add_capability("nip-90");

    // 5. Create handler info event (kind 31990)
    let content = serde_json::to_string(&handler_info.metadata).expect("should serialize metadata");

    let announcement_template = EventTemplate {
        kind: KIND_HANDLER_INFO,
        content,
        tags: handler_info.to_tags(),
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    };

    let announcement_event =
        finalize_event(&announcement_template, &provider_secret_key).expect("should sign");

    // 6. Connect to relay
    let relay = RelayConnection::new(&relay_url).expect("connection");
    relay.connect().await.expect("connect");

    // 7. Subscribe to handler info (consumer discovers providers)
    let filter = json!({
        "kinds": [KIND_HANDLER_INFO],
        "limit": 10
    });
    let mut rx = relay
        .subscribe_with_channel("provider-discovery", &[filter])
        .await
        .expect("subscribe");

    sleep(Duration::from_millis(100)).await;

    // 8. Publish provider announcement
    let confirmation = relay
        .publish_event(&announcement_event, Duration::from_secs(5))
        .await
        .expect("publish");

    assert!(confirmation.accepted);

    // 9. Consumer discovers the provider
    let received = tokio::time::timeout(Duration::from_secs(2), rx.recv())
        .await
        .expect("should receive within timeout")
        .expect("should have announcement");

    assert_eq!(received.id, announcement_event.id);
    assert_eq!(received.kind, KIND_HANDLER_INFO);

    // 10. Parse and verify metadata
    let parsed_metadata: HandlerMetadata =
        serde_json::from_str(&received.content).expect("should deserialize");

    assert_eq!(parsed_metadata.name, "OpenAgents Text Generator");
    assert_eq!(
        parsed_metadata.description,
        "High-quality text generation using local AI models"
    );
    assert_eq!(
        parsed_metadata.icon_url,
        Some("https://openagents.com/logo.png".to_string())
    );

    // Verify capability tags
    let has_text_gen = received
        .tags
        .iter()
        .any(|t| t[0] == "capability" && t[1] == "text-generation");
    assert!(has_text_gen, "Should have text-generation capability");

    let has_handler_type = received
        .tags
        .iter()
        .any(|t| t[0] == "handler" && t[1] == "compute_provider");
    assert!(
        has_handler_type,
        "Should have compute_provider handler type"
    );

    relay.disconnect().await.ok();
}

#[tokio::test]
async fn test_dvm_service_with_relay() {
    // Complete DVM workflow over a real relay:
    // 1. Provider announces capabilities (NIP-89)
    // 2. Customer publishes job request (NIP-90)
    // 3. Provider sends feedback (processing)
    // 4. Provider publishes result
    // 5. Customer receives result

    // 1. Start test relay
    let (_server, _tmp) = start_test_relay(19204).await;
    let relay_url = test_relay_url(19204);

    // 2. Create identities
    let provider_secret_key = generate_secret_key();
    let provider_pubkey = get_public_key(&provider_secret_key).expect("provider pubkey");
    let customer_secret_key = generate_secret_key();
    let customer_pubkey = get_public_key(&customer_secret_key).expect("customer pubkey");

    // 3. Connect both parties to relay
    let provider_relay = RelayConnection::new(&relay_url).expect("provider connection");
    provider_relay.connect().await.expect("provider connect");

    let customer_relay = RelayConnection::new(&relay_url).expect("customer connection");
    customer_relay.connect().await.expect("customer connect");

    // 4. Provider announces capabilities (NIP-89)
    let metadata = HandlerMetadata::new(
        "OpenAgents DVM",
        "Decentralized compute provider for text generation",
    )
    .with_website("https://openagents.com");

    let handler_info = HandlerInfo::new(
        hex::encode(provider_pubkey),
        HandlerType::ComputeProvider,
        metadata,
    )
    .add_capability("text-generation")
    .add_capability("code-generation");

    let announcement_template = EventTemplate {
        kind: KIND_HANDLER_INFO,
        content: serde_json::to_string(&handler_info.metadata).unwrap(),
        tags: handler_info.to_tags(),
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    };

    let announcement =
        finalize_event(&announcement_template, &provider_secret_key).expect("sign announcement");

    provider_relay
        .publish_event(&announcement, Duration::from_secs(5))
        .await
        .expect("publish announcement");

    // 5. Provider subscribes to job requests
    let job_filter = json!({
        "kinds": [KIND_JOB_TEXT_GENERATION],
        "limit": 10
    });
    let mut provider_rx = provider_relay
        .subscribe_with_channel("incoming-jobs", &[job_filter])
        .await
        .expect("provider subscribe");

    // 6. Customer subscribes to results for their pubkey
    let result_kind = KIND_JOB_TEXT_GENERATION + 1000;
    let result_filter = json!({
        "kinds": [result_kind, KIND_JOB_FEEDBACK],
        "#p": [hex::encode(customer_pubkey)],
        "limit": 10
    });
    let mut customer_rx = customer_relay
        .subscribe_with_channel("my-results", &[result_filter])
        .await
        .expect("customer subscribe");

    sleep(Duration::from_millis(100)).await;

    // 7. Customer submits job request
    let job_request = JobRequest::new(KIND_JOB_TEXT_GENERATION)
        .expect("create request")
        .add_input(JobInput::text(
            "Write a function to calculate fibonacci numbers",
        ))
        .add_param("language", "rust")
        .with_bid(5000);

    let request_template = EventTemplate {
        kind: job_request.kind,
        content: job_request.content.clone(),
        tags: job_request.to_tags(),
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    };

    let request_event =
        finalize_event(&request_template, &customer_secret_key).expect("sign request");

    customer_relay
        .publish_event(&request_event, Duration::from_secs(5))
        .await
        .expect("publish request");

    // 8. Provider receives job request
    let received_request = tokio::time::timeout(Duration::from_secs(2), provider_rx.recv())
        .await
        .expect("provider should receive request")
        .expect("should have request");

    assert_eq!(received_request.kind, KIND_JOB_TEXT_GENERATION);

    // 9. Provider sends "processing" feedback
    let processing_feedback = JobFeedback::new(
        JobStatus::Processing,
        received_request.id.clone(),
        hex::encode(customer_pubkey),
    )
    .with_status_extra("Generating code, ETA 30 seconds");

    let feedback_template = EventTemplate {
        kind: KIND_JOB_FEEDBACK,
        content: processing_feedback.content.clone(),
        tags: processing_feedback.to_tags(),
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    };

    let feedback_event =
        finalize_event(&feedback_template, &provider_secret_key).expect("sign feedback");

    provider_relay
        .publish_event(&feedback_event, Duration::from_secs(5))
        .await
        .expect("publish feedback");

    // 10. Customer receives feedback
    let received_feedback = tokio::time::timeout(Duration::from_secs(2), customer_rx.recv())
        .await
        .expect("customer should receive feedback")
        .expect("should have feedback");

    assert_eq!(received_feedback.kind, KIND_JOB_FEEDBACK);

    // 11. Provider publishes result
    let result_content = r#"fn fibonacci(n: u32) -> u64 {
    match n {
        0 => 0,
        1 => 1,
        n => fibonacci(n - 1) + fibonacci(n - 2),
    }
}"#;

    let job_result = JobResult::new(
        KIND_JOB_TEXT_GENERATION,
        received_request.id.clone(),
        hex::encode(customer_pubkey),
        result_content.to_string(),
    )
    .expect("create result");

    let result_template = EventTemplate {
        kind: job_result.kind,
        content: job_result.content.clone(),
        tags: job_result.to_tags(),
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    };

    let result_event = finalize_event(&result_template, &provider_secret_key).expect("sign result");

    provider_relay
        .publish_event(&result_event, Duration::from_secs(5))
        .await
        .expect("publish result");

    // 12. Customer receives result
    let received_result = tokio::time::timeout(Duration::from_secs(2), customer_rx.recv())
        .await
        .expect("customer should receive result")
        .expect("should have result");

    assert_eq!(received_result.kind, result_kind);
    assert_eq!(received_result.content, result_content);

    // Verify result references original request
    let has_request_ref = received_result
        .tags
        .iter()
        .any(|t| t[0] == "e" && t[1] == received_request.id);
    assert!(has_request_ref, "Result should reference request");

    // 13. Cleanup
    provider_relay.disconnect().await.ok();
    customer_relay.disconnect().await.ok();
}

// =============================================================================
// Type Validation Tests (kept from original for regression testing)
// =============================================================================

#[test]
fn test_job_request_structure() {
    let job_request = JobRequest::new(KIND_JOB_TEXT_GENERATION)
        .expect("should create job request")
        .add_input(JobInput::text("Test"))
        .add_param("model", "test")
        .with_bid(100);

    assert_eq!(job_request.kind, KIND_JOB_TEXT_GENERATION);
    assert_eq!(job_request.inputs.len(), 1);
    assert_eq!(job_request.params.len(), 1);
    assert_eq!(job_request.bid, Some(100));
}

#[test]
fn test_job_result_kind_calculation() {
    // Result kind should be request kind + 1000
    let expected_result_kind = KIND_JOB_TEXT_GENERATION + 1000;
    assert_eq!(expected_result_kind, 6050);
}

#[test]
fn test_job_status_variants() {
    // Verify all status variants exist
    let _processing = JobStatus::Processing;
    let _success = JobStatus::Success;
    let _error = JobStatus::Error;
    let _partial = JobStatus::Partial;
    let _payment_required = JobStatus::PaymentRequired;
}

#[test]
fn test_handler_type_compute_provider() {
    let handler_type = HandlerType::ComputeProvider;
    match handler_type {
        HandlerType::ComputeProvider => {}
        _ => panic!("Should be ComputeProvider"),
    }
}
