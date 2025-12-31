//! Integration tests for NIP-90 job request/result flow
//!
//! These tests verify the complete DVM job lifecycle:
//! - Job request creation and parsing
//! - Input/output handling
//! - Parameter management
//! - Job result creation
//! - Event validation

use crate::nip01::{EventTemplate, finalize_event, generate_secret_key, verify_event};
use crate::nip90::{
    InputType, JobInput, JobParam, JobRequest, JobResult, JobStatus, KIND_JOB_TEXT_GENERATION,
    is_job_request_kind, is_job_result_kind,
};

#[test]
fn test_job_request_creation() {
    // Create a text generation job request
    let request = JobRequest::new(KIND_JOB_TEXT_GENERATION)
        .unwrap()
        .add_input(JobInput::text("Write a haiku about Rust"))
        .add_param("temperature", "0.7")
        .add_param("max_tokens", "100")
        .with_bid(1000)
        .add_relay("wss://relay.damus.io");

    assert_eq!(request.kind, KIND_JOB_TEXT_GENERATION);
    assert_eq!(request.inputs.len(), 1);
    assert_eq!(request.params.len(), 2);
    assert_eq!(request.bid, Some(1000));
    assert_eq!(request.relays.len(), 1);
}

#[test]
fn test_job_request_to_event() {
    // Create job request
    let request = JobRequest::new(5050)
        .unwrap()
        .add_input(JobInput::text("Test prompt"))
        .add_param("model", "claude-3")
        .with_output("text/plain");

    // Convert to tags
    let tags = request.to_tags();

    // Verify tags contain expected elements
    assert!(
        tags.iter()
            .any(|tag| tag[0] == "i" && tag[1] == "Test prompt")
    );
    assert!(
        tags.iter()
            .any(|tag| tag[0] == "param" && tag[1] == "model")
    );
    assert!(
        tags.iter()
            .any(|tag| tag[0] == "output" && tag[1] == "text/plain")
    );

    // Create event template
    let template = EventTemplate {
        kind: request.kind,
        content: request.content,
        tags,
        created_at: chrono::Utc::now().timestamp() as u64,
    };

    // Sign the event
    let secret_key = generate_secret_key();
    let event = finalize_event(&template, &secret_key).unwrap();

    // Verify event is valid
    assert!(verify_event(&event).unwrap());
    assert_eq!(event.kind, 5050);
}

#[test]
fn test_job_request_round_trip() {
    // Create a job request
    let original = JobRequest::new(5050)
        .unwrap()
        .add_input(JobInput::text("Prompt"))
        .add_input(JobInput::url("https://example.com/data.txt"))
        .add_param("temperature", "0.8")
        .with_bid(5000);

    // Convert to event
    let tags = original.to_tags();
    let template = EventTemplate {
        kind: original.kind,
        content: String::new(),
        tags,
        created_at: chrono::Utc::now().timestamp() as u64,
    };

    let secret_key = generate_secret_key();
    let event = finalize_event(&template, &secret_key).unwrap();

    // Parse back from event
    let parsed = JobRequest::from_event(&event).unwrap();

    assert_eq!(parsed.kind, original.kind);
    assert_eq!(parsed.inputs.len(), original.inputs.len());
    assert_eq!(parsed.params.len(), original.params.len());
    assert_eq!(parsed.bid, original.bid);
}

#[test]
fn test_job_input_types() {
    let text = JobInput::text("Hello");
    assert_eq!(text.input_type, InputType::Text);
    assert_eq!(text.data, "Hello");

    let url = JobInput::url("https://example.com");
    assert_eq!(url.input_type, InputType::Url);
    assert_eq!(url.data, "https://example.com");

    let event = JobInput::event("event123", Some("wss://relay.com".to_string()));
    assert_eq!(event.input_type, InputType::Event);
    assert_eq!(event.relay, Some("wss://relay.com".to_string()));

    let job = JobInput::job("job456", None);
    assert_eq!(job.input_type, InputType::Job);
    assert_eq!(job.data, "job456");
}

#[test]
fn test_job_input_with_marker() {
    let input = JobInput::text("Primary input").with_marker("main");

    assert_eq!(input.marker, Some("main".to_string()));

    let tag = input.to_tag();
    assert_eq!(tag[0], "i");
    assert_eq!(tag[1], "Primary input");
    assert_eq!(tag[2], "text");
    assert_eq!(tag[4], "main");
}

#[test]
fn test_job_input_tag_conversion() {
    let input =
        JobInput::event("abc123", Some("wss://relay.com".to_string())).with_marker("source");

    let tag = input.to_tag();
    let parsed = JobInput::from_tag(&tag).unwrap();

    assert_eq!(parsed.data, input.data);
    assert_eq!(parsed.input_type, input.input_type);
    assert_eq!(parsed.relay, input.relay);
    assert_eq!(parsed.marker, input.marker);
}

#[test]
fn test_job_param_creation() {
    let param = JobParam::new("temperature", "0.9");

    assert_eq!(param.key, "temperature");
    assert_eq!(param.value, "0.9");

    let tag = param.to_tag();
    assert_eq!(tag, vec!["param", "temperature", "0.9"]);
}

#[test]
fn test_job_param_tag_conversion() {
    let param = JobParam::new("max_tokens", "2048");
    let tag = param.to_tag();
    let parsed = JobParam::from_tag(&tag).unwrap();

    assert_eq!(parsed.key, param.key);
    assert_eq!(parsed.value, param.value);
}

#[test]
fn test_job_result_creation() {
    let request_event_id = "abc123".to_string();
    let customer_pubkey = "customer_pk".to_string();

    let result = JobResult::new(
        KIND_JOB_TEXT_GENERATION,
        request_event_id.clone(),
        customer_pubkey.clone(),
        "The task is complete",
    )
    .unwrap()
    .with_amount(1000, Some("lnbc1000n...".to_string()));

    assert_eq!(result.kind, 6050);
    assert_eq!(result.request_id, request_event_id);
    assert_eq!(result.customer_pubkey, customer_pubkey);
    assert_eq!(result.content, "The task is complete");
    assert_eq!(result.amount, Some(1000));
    assert_eq!(result.bolt11, Some("lnbc1000n...".to_string()));
}

#[test]
fn test_job_result_to_event() {
    let result = JobResult::new(
        5050,
        "req123".to_string(),
        "customer".to_string(),
        "Result data",
    )
    .unwrap();

    let tags = result.to_tags();

    // Verify required tags
    assert!(tags.iter().any(|tag| tag[0] == "e" && tag[1] == "req123"));
    assert!(tags.iter().any(|tag| tag[0] == "p" && tag[1] == "customer"));
    assert!(
        tags.iter()
            .any(|tag| tag[0] == "status" && tag[1] == "success")
    );

    // Create and sign event
    let template = EventTemplate {
        kind: result.kind,
        content: result.content,
        tags,
        created_at: chrono::Utc::now().timestamp() as u64,
    };

    let secret_key = generate_secret_key();
    let event = finalize_event(&template, &secret_key).unwrap();

    assert!(verify_event(&event).unwrap());
    assert_eq!(event.kind, 6050);
    assert_eq!(event.content, "Result data");
}

#[test]
fn test_job_result_with_payment() {
    let result = JobResult::new(5050, "req".to_string(), "cust".to_string(), "")
        .unwrap()
        .with_amount(5000, Some("lnbc5000n1...".to_string()));

    let tags = result.to_tags();

    assert!(
        tags.iter()
            .any(|tag| tag[0] == "amount" && tag[1] == "5000")
    );
    assert!(
        tags.iter()
            .any(|tag| { tag[0] == "amount" && tag.len() == 3 && tag[2] == "lnbc5000n1..." })
    );
}

#[test]
fn test_job_result_round_trip() {
    let original = JobResult::new(
        5050,
        "request123".to_string(),
        "customer456".to_string(),
        "Result content",
    )
    .unwrap()
    .with_amount(3000, Some("lnbc3000n...".to_string()));

    // Convert to event
    let tags = original.to_tags();
    let template = EventTemplate {
        kind: original.kind,
        content: original.content.clone(),
        tags,
        created_at: chrono::Utc::now().timestamp() as u64,
    };

    let secret_key = generate_secret_key();
    let event = finalize_event(&template, &secret_key).unwrap();

    // Parse back
    let parsed = JobResult::from_event(&event).unwrap();

    assert_eq!(parsed.kind, original.kind);
    assert_eq!(parsed.request_id, original.request_id);
    assert_eq!(parsed.customer_pubkey, original.customer_pubkey);
    assert_eq!(parsed.content, original.content);
    assert_eq!(parsed.amount, original.amount);
}

#[test]
fn test_kind_validation() {
    // Valid job request kinds
    assert!(is_job_request_kind(5000));
    assert!(is_job_request_kind(5050));
    assert!(is_job_request_kind(5999));

    // Invalid job request kinds
    assert!(!is_job_request_kind(4999));
    assert!(!is_job_request_kind(6000));

    // Valid job result kinds
    assert!(is_job_result_kind(6000));
    assert!(is_job_result_kind(6050));
    assert!(is_job_result_kind(6999));

    // Invalid job result kinds
    assert!(!is_job_result_kind(5999));
    assert!(!is_job_result_kind(7000));
}

#[test]
fn test_job_request_creation_invalid_kind() {
    let result = JobRequest::new(4999);
    assert!(result.is_err());

    let result = JobRequest::new(6000);
    assert!(result.is_err());
}

#[test]
fn test_job_result_creation_invalid_kind() {
    let result = JobResult::new(4999, "req".to_string(), "cust".to_string(), "");
    assert!(result.is_err());

    let result = JobResult::new(6000, "req".to_string(), "cust".to_string(), "");
    assert!(result.is_err());
}

#[test]
fn test_job_status_conversion() {
    // Test status enum conversions
    assert_eq!(JobStatus::PaymentRequired.as_str(), "payment-required");
    assert_eq!(JobStatus::Processing.as_str(), "processing");
    assert_eq!(JobStatus::Success.as_str(), "success");
    assert_eq!(JobStatus::Error.as_str(), "error");
    assert_eq!(JobStatus::Partial.as_str(), "partial");

    // Test parsing
    assert_eq!(
        JobStatus::from_str("payment-required").unwrap(),
        JobStatus::PaymentRequired
    );
    assert_eq!(
        JobStatus::from_str("processing").unwrap(),
        JobStatus::Processing
    );
    assert_eq!(JobStatus::from_str("success").unwrap(), JobStatus::Success);
    assert_eq!(JobStatus::from_str("error").unwrap(), JobStatus::Error);
    assert_eq!(JobStatus::from_str("partial").unwrap(), JobStatus::Partial);
}

#[test]
fn test_multiple_inputs_request() {
    let request = JobRequest::new(5050)
        .unwrap()
        .add_input(JobInput::text("Primary prompt").with_marker("main"))
        .add_input(JobInput::url("https://example.com/context.txt"))
        .add_input(JobInput::event(
            "event789",
            Some("wss://relay.com".to_string()),
        ));

    assert_eq!(request.inputs.len(), 3);
    assert_eq!(request.inputs[0].marker, Some("main".to_string()));
    assert_eq!(request.inputs[1].input_type, InputType::Url);
    assert_eq!(request.inputs[2].input_type, InputType::Event);
}

#[test]
fn test_multiple_params_request() {
    let request = JobRequest::new(5050)
        .unwrap()
        .add_param("model", "llama3.2")
        .add_param("temperature", "0.7")
        .add_param("max_tokens", "2048")
        .add_param("top_p", "0.9");

    assert_eq!(request.params.len(), 4);
    assert_eq!(request.params[0].key, "model");
    assert_eq!(request.params[1].key, "temperature");
    assert_eq!(request.params[2].key, "max_tokens");
    assert_eq!(request.params[3].key, "top_p");
}

#[test]
fn test_service_provider_preferences() {
    let request = JobRequest::new(5050)
        .unwrap()
        .add_service_provider("provider1_pk")
        .add_service_provider("provider2_pk")
        .add_relay("wss://relay1.com")
        .add_relay("wss://relay2.com");

    assert_eq!(request.service_providers.len(), 2);
    assert_eq!(request.relays.len(), 2);

    let tags = request.to_tags();
    let p_tags: Vec<_> = tags.iter().filter(|tag| tag[0] == "p").collect();
    assert_eq!(p_tags.len(), 2);
}

#[test]
fn test_job_request_with_encrypted_params() {
    let mut request = JobRequest::new(5050).unwrap();
    request.encrypted = true;
    request.content = "encrypted_content_here".to_string();

    let tags = request.to_tags();
    assert!(tags.iter().any(|tag| tag[0] == "encrypted"));
}

#[test]
fn test_complete_job_lifecycle() {
    // 1. Customer creates job request
    let request = JobRequest::new(KIND_JOB_TEXT_GENERATION)
        .unwrap()
        .add_input(JobInput::text("Write a Rust function"))
        .add_param("language", "rust")
        .with_bid(2000);

    // 2. Convert to event and sign
    let customer_sk = generate_secret_key();
    let request_template = EventTemplate {
        kind: request.kind,
        content: request.content.clone(),
        tags: request.to_tags(),
        created_at: chrono::Utc::now().timestamp() as u64,
    };
    let request_event = finalize_event(&request_template, &customer_sk).unwrap();
    assert!(verify_event(&request_event).unwrap());

    // 3. Service provider processes and creates result
    let result = JobResult::new(
        request.kind,
        request_event.id.clone(),
        hex::encode(request_event.pubkey),
        "fn add(a: i32, b: i32) -> i32 { a + b }",
    )
    .unwrap()
    .with_amount(2000, Some("lnbc2000n1...".to_string()));

    // 4. Service provider signs and publishes result
    let provider_sk = generate_secret_key();
    let result_template = EventTemplate {
        kind: result.kind,
        content: result.content.clone(),
        tags: result.to_tags(),
        created_at: chrono::Utc::now().timestamp() as u64,
    };
    let result_event = finalize_event(&result_template, &provider_sk).unwrap();
    assert!(verify_event(&result_event).unwrap());

    // 5. Verify result references original request
    assert_eq!(result.kind, request.kind + 1000);
    assert!(
        result_event
            .tags
            .iter()
            .any(|tag| { tag.len() >= 2 && tag[0] == "e" && tag[1] == request_event.id })
    );
}
