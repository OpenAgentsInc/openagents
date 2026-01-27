//! Integration tests for NIP-90 DVM job types and workflows

use nostr::{
    InputType, JobFeedback, JobInput, JobParam, JobRequest, JobResult, JobStatus,
    KIND_JOB_IMAGE_GENERATION, KIND_JOB_SPEECH_TO_TEXT, KIND_JOB_SUMMARIZATION,
    KIND_JOB_TEXT_EXTRACTION, KIND_JOB_TEXT_GENERATION, KIND_JOB_TRANSLATION,
};
use std::str::FromStr;

// =========================================================================
// InputType tests
// =========================================================================

#[test]
fn test_input_type_serialization() {
    assert_eq!(InputType::Text.as_str(), "text");
    assert_eq!(InputType::Url.as_str(), "url");
    assert_eq!(InputType::Event.as_str(), "event");
    assert_eq!(InputType::Job.as_str(), "job");
}

#[test]
fn test_input_type_case_insensitive_parsing() {
    assert!(matches!(
        InputType::from_str("TEXT"),
        Ok(InputType::Text)
    ));
    assert!(matches!(
        InputType::from_str("Text"),
        Ok(InputType::Text)
    ));
    assert!(matches!(InputType::from_str("url"), Ok(InputType::Url)));
    assert!(matches!(InputType::from_str("URL"), Ok(InputType::Url)));
}

#[test]
fn test_input_type_invalid() {
    assert!(InputType::from_str("invalid").is_err());
    assert!(InputType::from_str("").is_err());
    assert!(InputType::from_str("link").is_err());
}

// =========================================================================
// JobStatus tests
// =========================================================================

#[test]
fn test_job_status_serialization() {
    assert_eq!(JobStatus::PaymentRequired.as_str(), "payment-required");
    assert_eq!(JobStatus::Processing.as_str(), "processing");
    assert_eq!(JobStatus::Error.as_str(), "error");
    assert_eq!(JobStatus::Success.as_str(), "success");
    assert_eq!(JobStatus::Partial.as_str(), "partial");
}

#[test]
fn test_job_status_parsing() {
    assert!(matches!(
        JobStatus::from_str("payment-required"),
        Ok(JobStatus::PaymentRequired)
    ));
    assert!(matches!(
        JobStatus::from_str("processing"),
        Ok(JobStatus::Processing)
    ));
    assert!(matches!(JobStatus::from_str("error"), Ok(JobStatus::Error)));
    assert!(matches!(
        JobStatus::from_str("success"),
        Ok(JobStatus::Success)
    ));
    assert!(matches!(
        JobStatus::from_str("partial"),
        Ok(JobStatus::Partial)
    ));
}

#[test]
fn test_job_status_invalid() {
    assert!(JobStatus::from_str("invalid").is_err());
    assert!(JobStatus::from_str("").is_err());
    assert!(JobStatus::from_str("PROCESSING").is_err()); // Case sensitive
}

// =========================================================================
// JobInput construction and tag conversion
// =========================================================================

#[test]
fn test_job_input_text_creation() {
    let input = JobInput::text("Summarize this text");
    assert_eq!(input.data, "Summarize this text");
    assert_eq!(input.input_type, InputType::Text);
    assert!(input.relay.is_none());
    assert!(input.marker.is_none());
}

#[test]
fn test_job_input_url_creation() {
    let input = JobInput::url("https://example.com/image.png");
    assert_eq!(input.data, "https://example.com/image.png");
    assert_eq!(input.input_type, InputType::Url);
    assert!(input.relay.is_none());
}

#[test]
fn test_job_input_event_creation() {
    let input = JobInput::event("event123abc", Some("wss://relay.damus.io".to_string()));
    assert_eq!(input.data, "event123abc");
    assert_eq!(input.input_type, InputType::Event);
    assert_eq!(input.relay, Some("wss://relay.damus.io".to_string()));
}

#[test]
fn test_job_input_job_creation() {
    let input = JobInput::job("job456def", Some("wss://relay.nostr.band".to_string()));
    assert_eq!(input.data, "job456def");
    assert_eq!(input.input_type, InputType::Job);
    assert_eq!(input.relay, Some("wss://relay.nostr.band".to_string()));
}

#[test]
fn test_job_input_with_marker() {
    let input = JobInput::text("content").with_marker("source");
    assert_eq!(input.marker, Some("source".to_string()));

    let input = JobInput::url("https://example.com").with_marker("audio");
    assert_eq!(input.data, "https://example.com");
    assert_eq!(input.marker, Some("audio".to_string()));
}

#[test]
fn test_job_input_to_tag_simple() {
    let input = JobInput::text("Hello world");
    let tag = input.to_tag();
    assert_eq!(tag, vec!["i", "Hello world", "text"]);
}

#[test]
fn test_job_input_to_tag_with_relay() {
    let input = JobInput::event("abc123", Some("wss://relay.com".to_string()));
    let tag = input.to_tag();
    assert_eq!(tag, vec!["i", "abc123", "event", "wss://relay.com"]);
}

#[test]
fn test_job_input_to_tag_with_marker_no_relay() {
    let input = JobInput::text("data").with_marker("primary");
    let tag = input.to_tag();
    // When marker is present but relay is not, relay becomes empty string
    assert_eq!(tag, vec!["i", "data", "text", "", "primary"]);
}

#[test]
fn test_job_input_to_tag_with_relay_and_marker() {
    let input =
        JobInput::event("xyz789", Some("wss://relay.io".to_string())).with_marker("reference");
    let tag = input.to_tag();
    assert_eq!(
        tag,
        vec!["i", "xyz789", "event", "wss://relay.io", "reference"]
    );
}

#[test]
fn test_job_input_from_tag_minimal() {
    let tag = vec!["i".to_string(), "data".to_string(), "text".to_string()];
    let input = JobInput::from_tag(&tag).unwrap();
    assert_eq!(input.data, "data");
    assert_eq!(input.input_type, InputType::Text);
    assert!(input.relay.is_none());
    assert!(input.marker.is_none());
}

#[test]
fn test_job_input_from_tag_with_relay() {
    let tag = vec![
        "i".to_string(),
        "event123".to_string(),
        "event".to_string(),
        "wss://relay.com".to_string(),
    ];
    let input = JobInput::from_tag(&tag).unwrap();
    assert_eq!(input.data, "event123");
    assert_eq!(input.input_type, InputType::Event);
    assert_eq!(input.relay, Some("wss://relay.com".to_string()));
}

#[test]
fn test_job_input_from_tag_with_empty_relay() {
    let tag = vec![
        "i".to_string(),
        "data".to_string(),
        "text".to_string(),
        "".to_string(),
        "marker".to_string(),
    ];
    let input = JobInput::from_tag(&tag).unwrap();
    assert!(input.relay.is_none()); // Empty string filtered out
    assert_eq!(input.marker, Some("marker".to_string()));
}

#[test]
fn test_job_input_from_tag_invalid() {
    // Too few elements
    let tag = vec!["i".to_string(), "data".to_string()];
    assert!(JobInput::from_tag(&tag).is_err());

    // Wrong tag type
    let tag = vec!["param".to_string(), "data".to_string(), "text".to_string()];
    assert!(JobInput::from_tag(&tag).is_err());
}

#[test]
fn test_job_input_roundtrip() {
    let original =
        JobInput::event("event123", Some("wss://relay.com".to_string())).with_marker("source");
    let tag = original.to_tag();
    let recovered = JobInput::from_tag(&tag).unwrap();

    assert_eq!(original.data, recovered.data);
    assert_eq!(original.input_type, recovered.input_type);
    assert_eq!(original.relay, recovered.relay);
    assert_eq!(original.marker, recovered.marker);
}

// =========================================================================
// JobParam tests
// =========================================================================

#[test]
fn test_job_param_creation() {
    let param = JobParam::new("model", "llama3.2");
    assert_eq!(param.key, "model");
    assert_eq!(param.value, "llama3.2");
}

#[test]
fn test_job_param_to_tag() {
    let param = JobParam::new("temperature", "0.7");
    let tag = param.to_tag();
    assert_eq!(tag, vec!["param", "temperature", "0.7"]);
}

#[test]
fn test_job_param_from_tag() {
    let tag = vec![
        "param".to_string(),
        "max_tokens".to_string(),
        "2048".to_string(),
    ];
    let param = JobParam::from_tag(&tag).unwrap();
    assert_eq!(param.key, "max_tokens");
    assert_eq!(param.value, "2048");
}

#[test]
fn test_job_param_from_tag_invalid() {
    // Too few elements
    let tag = vec!["param".to_string(), "key".to_string()];
    assert!(JobParam::from_tag(&tag).is_err());

    // Wrong tag type
    let tag = vec!["i".to_string(), "key".to_string(), "value".to_string()];
    assert!(JobParam::from_tag(&tag).is_err());
}

#[test]
fn test_job_param_roundtrip() {
    let original = JobParam::new("language", "fr");
    let tag = original.to_tag();
    let recovered = JobParam::from_tag(&tag).unwrap();

    assert_eq!(original.key, recovered.key);
    assert_eq!(original.value, recovered.value);
}

// =========================================================================
// JobRequest tests
// =========================================================================

#[test]
fn test_job_request_text_generation() {
    let request = JobRequest::new(KIND_JOB_TEXT_GENERATION)
        .unwrap()
        .add_input(JobInput::text("What is the capital of France?"))
        .add_param("model", "llama3.2")
        .add_param("temperature", "0.7")
        .with_output("text/plain")
        .with_bid(1000);

    assert_eq!(request.kind, 5050);
    assert_eq!(request.inputs.len(), 1);
    assert_eq!(request.params.len(), 2);
    assert_eq!(request.output, Some("text/plain".to_string()));
    assert_eq!(request.bid, Some(1000));
}

#[test]
fn test_job_request_image_generation() {
    let request = JobRequest::new(KIND_JOB_IMAGE_GENERATION)
        .unwrap()
        .add_input(JobInput::text("A sunset over mountains"))
        .add_param("style", "photorealistic")
        .add_param("size", "1024x1024")
        .with_output("image/png")
        .with_bid(5000);

    assert_eq!(request.kind, 5100);
    assert_eq!(request.result_kind(), 6100);
}

#[test]
fn test_job_request_with_relays() {
    let request = JobRequest::new(KIND_JOB_SUMMARIZATION)
        .unwrap()
        .add_relay("wss://relay.damus.io")
        .add_relay("wss://relay.nostr.band");

    assert_eq!(request.relays.len(), 2);
    assert_eq!(request.relays[0], "wss://relay.damus.io");
}

#[test]
fn test_job_request_with_service_providers() {
    let request = JobRequest::new(KIND_JOB_TRANSLATION)
        .unwrap()
        .add_service_provider("npub1abc")
        .add_service_provider("npub2def");

    assert_eq!(request.service_providers.len(), 2);
}

#[test]
fn test_job_request_invalid_kind() {
    assert!(JobRequest::new(4999).is_err()); // Too low
    assert!(JobRequest::new(6000).is_err()); // Result kind, not request
    assert!(JobRequest::new(7000).is_err()); // Feedback kind
}

#[test]
fn test_job_request_result_kind_calculation() {
    let request = JobRequest::new(5000).unwrap();
    assert_eq!(request.result_kind(), 6000);

    let request = JobRequest::new(5050).unwrap();
    assert_eq!(request.result_kind(), 6050);

    let request = JobRequest::new(5999).unwrap();
    assert_eq!(request.result_kind(), 6999);
}

#[test]
fn test_job_request_to_tags_comprehensive() {
    let request = JobRequest::new(KIND_JOB_TEXT_GENERATION)
        .unwrap()
        .add_input(JobInput::text("Test input"))
        .add_input(JobInput::url("https://example.com"))
        .add_param("model", "gpt-4")
        .add_param("temperature", "0.8")
        .with_output("application/json")
        .with_bid(2500)
        .add_relay("wss://relay1.com")
        .add_relay("wss://relay2.com")
        .add_service_provider("provider123");

    let tags = request.to_tags();

    // Check for input tags
    assert!(
        tags.iter()
            .any(|t| t[0] == "i" && t[1] == "Test input" && t[2] == "text")
    );
    assert!(
        tags.iter()
            .any(|t| t[0] == "i" && t[1] == "https://example.com" && t[2] == "url")
    );

    // Check for param tags
    assert!(
        tags.iter()
            .any(|t| t[0] == "param" && t[1] == "model" && t[2] == "gpt-4")
    );
    assert!(
        tags.iter()
            .any(|t| t[0] == "param" && t[1] == "temperature" && t[2] == "0.8")
    );

    // Check for output tag
    assert!(
        tags.iter()
            .any(|t| t[0] == "output" && t[1] == "application/json")
    );

    // Check for bid tag
    assert!(tags.iter().any(|t| t[0] == "bid" && t[1] == "2500"));

    // Check for relays tag
    assert!(tags.iter().any(|t| {
        t[0] == "relays"
            && t.contains(&"wss://relay1.com".to_string())
            && t.contains(&"wss://relay2.com".to_string())
    }));

    // Check for service provider tag
    assert!(tags.iter().any(|t| t[0] == "p" && t[1] == "provider123"));
}

#[test]
fn test_job_request_encrypted_flag() {
    let mut request = JobRequest::new(KIND_JOB_TEXT_GENERATION).unwrap();
    request.encrypted = true;
    request.content = "encrypted_payload".to_string();

    let tags = request.to_tags();
    assert!(tags.iter().any(|t| t[0] == "encrypted"));
}

// =========================================================================
// JobResult tests
// =========================================================================

#[test]
fn test_job_result_creation() {
    let result = JobResult::new(5050, "request123", "customer456", "The answer is 42.").unwrap();

    assert_eq!(result.kind, 6050);
    assert_eq!(result.request_id, "request123");
    assert_eq!(result.customer_pubkey, "customer456");
    assert_eq!(result.content, "The answer is 42.");
}

#[test]
fn test_job_result_with_payment() {
    let result = JobResult::new(5001, "req123", "cust456", "Result")
        .unwrap()
        .with_amount(1500, Some("lnbc1500n1...".to_string()));

    assert_eq!(result.amount, Some(1500));
    assert_eq!(result.bolt11, Some("lnbc1500n1...".to_string()));
}

#[test]
fn test_job_result_with_inputs() {
    let result = JobResult::new(5050, "req123", "cust456", "Output")
        .unwrap()
        .add_input(JobInput::text("Original input 1"))
        .add_input(JobInput::text("Original input 2"));

    assert_eq!(result.inputs.len(), 2);
}

#[test]
fn test_job_result_invalid_request_kind() {
    assert!(JobResult::new(4999, "req", "cust", "content").is_err());
    assert!(JobResult::new(6000, "req", "cust", "content").is_err()); // Result kind, not request
}

#[test]
fn test_job_result_to_tags() {
    let result = JobResult::new(5050, "req123", "cust456", "Result content")
        .unwrap()
        .with_request(r#"{"kind":5050}"#)
        .with_request_relay("wss://relay.com")
        .add_input(JobInput::text("input"))
        .with_amount(1000, Some("lnbc...".to_string()));

    let tags = result.to_tags();

    // Check for request tag
    assert!(tags.iter().any(|t| t[0] == "request"));

    // Check for event reference
    assert!(
        tags.iter()
            .any(|t| t[0] == "e" && t[1] == "req123" && t[2] == "wss://relay.com")
    );

    // Check for input tag
    assert!(
        tags.iter()
            .any(|t| t[0] == "i" && t[1] == "input" && t[2] == "text")
    );

    // Check for customer pubkey
    assert!(tags.iter().any(|t| t[0] == "p" && t[1] == "cust456"));

    // Check for amount tag
    assert!(
        tags.iter()
            .any(|t| t[0] == "amount" && t[1] == "1000" && t[2] == "lnbc...")
    );
}

#[test]
fn test_job_result_encrypted_flag() {
    let mut result = JobResult::new(5050, "req", "cust", "encrypted_content").unwrap();
    result.encrypted = true;

    let tags = result.to_tags();
    assert!(tags.iter().any(|t| t[0] == "encrypted"));
}

// =========================================================================
// JobFeedback tests
// =========================================================================

#[test]
fn test_job_feedback_processing() {
    let feedback = JobFeedback::new(JobStatus::Processing, "req123", "cust456");

    assert_eq!(feedback.status, JobStatus::Processing);
    assert_eq!(feedback.request_id, "req123");
    assert_eq!(feedback.customer_pubkey, "cust456");
}

#[test]
fn test_job_feedback_payment_required() {
    let feedback = JobFeedback::new(JobStatus::PaymentRequired, "req123", "cust456")
        .with_status_extra("Please pay 5000 msats to continue")
        .with_amount(5000, Some("lnbc5000n1...".to_string()));

    assert_eq!(feedback.status, JobStatus::PaymentRequired);
    assert_eq!(
        feedback.status_extra,
        Some("Please pay 5000 msats to continue".to_string())
    );
    assert_eq!(feedback.amount, Some(5000));
}

#[test]
fn test_job_feedback_error() {
    let feedback = JobFeedback::new(JobStatus::Error, "req123", "cust456")
        .with_status_extra("Model not available");

    assert_eq!(feedback.status, JobStatus::Error);
    assert_eq!(
        feedback.status_extra,
        Some("Model not available".to_string())
    );
}

#[test]
fn test_job_feedback_partial_results() {
    let feedback = JobFeedback::new(JobStatus::Partial, "req123", "cust456")
        .with_content("Here are the first 100 results...");

    assert_eq!(feedback.status, JobStatus::Partial);
    assert_eq!(feedback.content, "Here are the first 100 results...");
}

#[test]
fn test_job_feedback_success() {
    let feedback = JobFeedback::new(JobStatus::Success, "req123", "cust456")
        .with_status_extra("Job completed successfully");

    assert_eq!(feedback.status, JobStatus::Success);
}

#[test]
fn test_job_feedback_to_tags() {
    let feedback = JobFeedback::new(JobStatus::PaymentRequired, "req123", "cust456")
        .with_status_extra("Payment needed")
        .with_request_relay("wss://relay.com")
        .with_amount(2000, Some("lnbc...".to_string()));

    let tags = feedback.to_tags();

    // Check for status tag
    assert!(
        tags.iter()
            .any(|t| t[0] == "status" && t[1] == "payment-required" && t[2] == "Payment needed")
    );

    // Check for event reference
    assert!(
        tags.iter()
            .any(|t| t[0] == "e" && t[1] == "req123" && t[2] == "wss://relay.com")
    );

    // Check for customer pubkey
    assert!(tags.iter().any(|t| t[0] == "p" && t[1] == "cust456"));

    // Check for amount tag
    assert!(
        tags.iter()
            .any(|t| t[0] == "amount" && t[1] == "2000" && t[2] == "lnbc...")
    );
}

// =========================================================================
// Workflow integration tests
// =========================================================================

#[test]
fn test_complete_dvm_workflow() {
    // 1. Customer creates job request
    let request = JobRequest::new(KIND_JOB_TEXT_GENERATION)
        .unwrap()
        .add_input(JobInput::text("Explain quantum computing"))
        .add_param("model", "llama3.2")
        .add_param("max_tokens", "512")
        .with_bid(1000)
        .add_relay("wss://relay.damus.io");

    assert_eq!(request.kind, 5050);
    let tags = request.to_tags();
    assert!(!tags.is_empty());

    // 2. Service provider sends processing feedback
    let feedback = JobFeedback::new(JobStatus::Processing, "req_event_123", "customer_pubkey");
    assert_eq!(feedback.status, JobStatus::Processing);

    // 3. Service provider completes job
    let result = JobResult::new(
        5050,
        "req_event_123",
        "customer_pubkey",
        "Quantum computing uses quantum bits...",
    )
    .unwrap()
    .with_amount(500, Some("lnbc500n1...".to_string()));

    assert_eq!(result.kind, 6050);
    assert_eq!(result.amount, Some(500));
}

#[test]
fn test_job_chaining_workflow() {
    // Job 1: Speech to text
    let job1 = JobRequest::new(KIND_JOB_SPEECH_TO_TEXT)
        .unwrap()
        .add_input(JobInput::url("https://example.com/podcast.mp3"));

    assert_eq!(job1.kind, 5250);

    // Job 1 result
    let result1 = JobResult::new(
        5250,
        "job1_event_id",
        "customer_pubkey",
        "Transcribed text content...",
    )
    .unwrap();

    assert_eq!(result1.kind, 6250);

    // Job 2: Summarization (uses output of job 1)
    let job2 = JobRequest::new(KIND_JOB_SUMMARIZATION)
        .unwrap()
        .add_input(JobInput::job(
            "job1_event_id",
            Some("wss://relay.damus.io".to_string()),
        ));

    assert_eq!(job2.kind, 5001);
    assert_eq!(job2.inputs[0].input_type, InputType::Job);
    assert_eq!(job2.inputs[0].data, "job1_event_id");
}

#[test]
fn test_multi_input_translation_job() {
    let request = JobRequest::new(KIND_JOB_TRANSLATION)
        .unwrap()
        .add_input(JobInput::text("Hello world").with_marker("source"))
        .add_input(JobInput::text("French").with_marker("target_language"))
        .add_param("preserve_formatting", "true");

    assert_eq!(request.kind, 5002);
    assert_eq!(request.inputs.len(), 2);
    assert_eq!(request.inputs[0].marker, Some("source".to_string()));
    assert_eq!(
        request.inputs[1].marker,
        Some("target_language".to_string())
    );
}

#[test]
fn test_text_extraction_from_url() {
    let request = JobRequest::new(KIND_JOB_TEXT_EXTRACTION)
        .unwrap()
        .add_input(JobInput::url("https://example.com/document.pdf"))
        .with_output("text/plain");

    assert_eq!(request.kind, 5000);
    assert_eq!(request.inputs[0].input_type, InputType::Url);
}

#[test]
fn test_payment_required_feedback_workflow() {
    let feedback = JobFeedback::new(JobStatus::PaymentRequired, "req123", "cust456")
        .with_status_extra("Insufficient balance")
        .with_amount(10000, Some("lnbc10000n1...".to_string()));

    let tags = feedback.to_tags();

    // Verify payment invoice is in tags
    assert!(tags.iter().any(|t| t[0] == "amount" && t[1] == "10000"));
}

#[test]
fn test_error_feedback_workflow() {
    let feedback = JobFeedback::new(JobStatus::Error, "req123", "cust456")
        .with_status_extra("Model timeout after 30s");

    let tags = feedback.to_tags();

    assert!(tags.iter().any(|t| t[0] == "status" && t[1] == "error"));
}
