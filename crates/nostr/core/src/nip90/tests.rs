use super::*;
use std::str::FromStr;

// =========================================================================
// Kind validation tests
// =========================================================================

#[test]
fn test_is_job_request_kind() {
    assert!(is_job_request_kind(5000));
    assert!(is_job_request_kind(5001));
    assert!(is_job_request_kind(5500));
    assert!(is_job_request_kind(5999));

    assert!(!is_job_request_kind(4999));
    assert!(!is_job_request_kind(6000));
    assert!(!is_job_request_kind(7000));
}

#[test]
fn test_is_job_result_kind() {
    assert!(is_job_result_kind(6000));
    assert!(is_job_result_kind(6001));
    assert!(is_job_result_kind(6500));
    assert!(is_job_result_kind(6999));

    assert!(!is_job_result_kind(5999));
    assert!(!is_job_result_kind(7000));
    assert!(!is_job_result_kind(7001));
}

#[test]
fn test_is_job_feedback_kind() {
    assert!(is_job_feedback_kind(7000));

    assert!(!is_job_feedback_kind(6999));
    assert!(!is_job_feedback_kind(7001));
}

#[test]
fn test_is_dvm_kind() {
    // Job requests
    assert!(is_dvm_kind(5000));
    assert!(is_dvm_kind(5999));

    // Job results
    assert!(is_dvm_kind(6000));
    assert!(is_dvm_kind(6999));

    // Job feedback
    assert!(is_dvm_kind(7000));

    // Not DVM
    assert!(!is_dvm_kind(4999));
    assert!(!is_dvm_kind(7001));
    assert!(!is_dvm_kind(1));
}

#[test]
fn test_get_result_kind() {
    assert_eq!(get_result_kind(5000), Some(6000));
    assert_eq!(get_result_kind(5001), Some(6001));
    assert_eq!(get_result_kind(5999), Some(6999));

    assert_eq!(get_result_kind(4999), None);
    assert_eq!(get_result_kind(6000), None);
}

#[test]
fn test_get_request_kind() {
    assert_eq!(get_request_kind(6000), Some(5000));
    assert_eq!(get_request_kind(6001), Some(5001));
    assert_eq!(get_request_kind(6999), Some(5999));

    assert_eq!(get_request_kind(5999), None);
    assert_eq!(get_request_kind(7000), None);
}

// =========================================================================
// InputType tests
// =========================================================================

#[test]
fn test_input_type_as_str() {
    assert_eq!(InputType::Url.as_str(), "url");
    assert_eq!(InputType::Event.as_str(), "event");
    assert_eq!(InputType::Job.as_str(), "job");
    assert_eq!(InputType::Text.as_str(), "text");
}

#[test]
fn test_input_type_from_str() {
    assert!(matches!(InputType::from_str("url"), Ok(InputType::Url)));
    assert!(matches!(InputType::from_str("event"), Ok(InputType::Event)));
    assert!(matches!(InputType::from_str("job"), Ok(InputType::Job)));
    assert!(matches!(InputType::from_str("text"), Ok(InputType::Text)));

    // Case insensitive
    assert!(matches!(InputType::from_str("URL"), Ok(InputType::Url)));
    assert!(matches!(InputType::from_str("Text"), Ok(InputType::Text)));

    // Invalid
    assert!(InputType::from_str("invalid").is_err());
}

// =========================================================================
// JobStatus tests
// =========================================================================

#[test]
fn test_job_status_as_str() {
    assert_eq!(JobStatus::PaymentRequired.as_str(), "payment-required");
    assert_eq!(JobStatus::Processing.as_str(), "processing");
    assert_eq!(JobStatus::Error.as_str(), "error");
    assert_eq!(JobStatus::Success.as_str(), "success");
    assert_eq!(JobStatus::Partial.as_str(), "partial");
}

#[test]
fn test_job_status_from_str() {
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

    // Invalid
    assert!(JobStatus::from_str("invalid").is_err());
}

// =========================================================================
// JobInput tests
// =========================================================================

#[test]
fn test_job_input_text() {
    let input = JobInput::text("What is the capital of France?");
    assert_eq!(input.data, "What is the capital of France?");
    assert_eq!(input.input_type, InputType::Text);
    assert!(input.relay.is_none());
    assert!(input.marker.is_none());
}

#[test]
fn test_job_input_url() {
    let input = JobInput::url("https://example.com/audio.mp3");
    assert_eq!(input.data, "https://example.com/audio.mp3");
    assert_eq!(input.input_type, InputType::Url);
}

#[test]
fn test_job_input_event() {
    let input = JobInput::event("abc123", Some("wss://relay.example.com".to_string()));
    assert_eq!(input.data, "abc123");
    assert_eq!(input.input_type, InputType::Event);
    assert_eq!(input.relay, Some("wss://relay.example.com".to_string()));
}

#[test]
fn test_job_input_job() {
    let input = JobInput::job("def456", Some("wss://relay.example.com".to_string()));
    assert_eq!(input.data, "def456");
    assert_eq!(input.input_type, InputType::Job);
}

#[test]
fn test_job_input_with_marker() {
    let input = JobInput::text("some text").with_marker("source");
    assert_eq!(input.marker, Some("source".to_string()));
}

#[test]
fn test_job_input_to_tag() {
    let input = JobInput::text("Hello");
    let tag = input.to_tag();
    assert_eq!(tag, vec!["i", "Hello", "text"]);

    let input = JobInput::url("https://example.com").with_marker("audio");
    let tag = input.to_tag();
    assert_eq!(tag, vec!["i", "https://example.com", "url", "", "audio"]);

    let input = JobInput::event("abc123", Some("wss://relay.com".to_string()));
    let tag = input.to_tag();
    assert_eq!(tag, vec!["i", "abc123", "event", "wss://relay.com"]);
}

#[test]
fn test_job_input_from_tag() {
    let tag = vec!["i".to_string(), "Hello".to_string(), "text".to_string()];
    let input = JobInput::from_tag(&tag).unwrap();
    assert_eq!(input.data, "Hello");
    assert_eq!(input.input_type, InputType::Text);

    let tag = vec![
        "i".to_string(),
        "abc123".to_string(),
        "event".to_string(),
        "wss://relay.com".to_string(),
        "source".to_string(),
    ];
    let input = JobInput::from_tag(&tag).unwrap();
    assert_eq!(input.data, "abc123");
    assert_eq!(input.input_type, InputType::Event);
    assert_eq!(input.relay, Some("wss://relay.com".to_string()));
    assert_eq!(input.marker, Some("source".to_string()));
}

// =========================================================================
// JobParam tests
// =========================================================================

#[test]
fn test_job_param() {
    let param = JobParam::new("model", "gpt-4");
    assert_eq!(param.key, "model");
    assert_eq!(param.value, "gpt-4");
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
        "512".to_string(),
    ];
    let param = JobParam::from_tag(&tag).unwrap();
    assert_eq!(param.key, "max_tokens");
    assert_eq!(param.value, "512");
}

// =========================================================================
// JobRequest tests
// =========================================================================

#[test]
fn test_job_request_new() {
    let request = JobRequest::new(5001).unwrap();
    assert_eq!(request.kind, 5001);
    assert!(request.inputs.is_empty());
    assert!(request.params.is_empty());
}

#[test]
fn test_job_request_invalid_kind() {
    assert!(JobRequest::new(4999).is_err());
    assert!(JobRequest::new(6000).is_err());
}

#[test]
fn test_job_request_builder() {
    let request = JobRequest::new(KIND_JOB_TEXT_GENERATION)
        .unwrap()
        .add_input(JobInput::text("What is the capital of France?"))
        .with_output("text/plain")
        .add_param("model", "gpt-4")
        .add_param("temperature", "0.7")
        .with_bid(1000)
        .add_relay("wss://relay.example.com")
        .add_service_provider("abc123");

    assert_eq!(request.kind, 5050);
    assert_eq!(request.inputs.len(), 1);
    assert_eq!(request.output, Some("text/plain".to_string()));
    assert_eq!(request.params.len(), 2);
    assert_eq!(request.bid, Some(1000));
    assert_eq!(request.relays, vec!["wss://relay.example.com"]);
    assert_eq!(request.service_providers, vec!["abc123"]);
}

#[test]
fn test_job_request_result_kind() {
    let request = JobRequest::new(5001).unwrap();
    assert_eq!(request.result_kind(), 6001);

    let request = JobRequest::new(5050).unwrap();
    assert_eq!(request.result_kind(), 6050);
}

#[test]
fn test_job_request_to_tags() {
    let request = JobRequest::new(5001)
        .unwrap()
        .add_input(JobInput::text("Hello"))
        .with_output("text/plain")
        .add_param("lang", "es")
        .with_bid(5000)
        .add_relay("wss://relay1.com")
        .add_relay("wss://relay2.com");

    let tags = request.to_tags();

    assert!(
        tags.iter()
            .any(|t| t[0] == "i" && t[1] == "Hello" && t[2] == "text")
    );
    assert!(
        tags.iter()
            .any(|t| t[0] == "output" && t[1] == "text/plain")
    );
    assert!(
        tags.iter()
            .any(|t| t[0] == "param" && t[1] == "lang" && t[2] == "es")
    );
    assert!(tags.iter().any(|t| t[0] == "bid" && t[1] == "5000"));
    assert!(
        tags.iter()
            .any(|t| t[0] == "relays" && t.contains(&"wss://relay1.com".to_string()))
    );
}

// =========================================================================
// JobResult tests
// =========================================================================

#[test]
fn test_job_result_new() {
    let result =
        JobResult::new(5001, "request123", "customer456", "The capital is Paris.").unwrap();

    assert_eq!(result.kind, 6001);
    assert_eq!(result.request_id, "request123");
    assert_eq!(result.customer_pubkey, "customer456");
    assert_eq!(result.content, "The capital is Paris.");
}

#[test]
fn test_job_result_builder() {
    let result = JobResult::new(5001, "req123", "cust456", "Result content")
        .unwrap()
        .with_request(r#"{"kind":5001}"#)
        .with_request_relay("wss://relay.com")
        .add_input(JobInput::text("Original input"))
        .with_amount(1000, Some("lnbc...".to_string()));

    assert_eq!(result.request, Some(r#"{"kind":5001}"#.to_string()));
    assert_eq!(result.request_relay, Some("wss://relay.com".to_string()));
    assert_eq!(result.inputs.len(), 1);
    assert_eq!(result.amount, Some(1000));
    assert_eq!(result.bolt11, Some("lnbc...".to_string()));
}

#[test]
fn test_job_result_to_tags() {
    let result = JobResult::new(5001, "req123", "cust456", "Result")
        .unwrap()
        .with_request(r#"{"kind":5001}"#)
        .with_request_relay("wss://relay.com")
        .with_amount(1000, None);

    let tags = result.to_tags();

    assert!(tags.iter().any(|t| t[0] == "request"));
    assert!(
        tags.iter()
            .any(|t| t[0] == "e" && t[1] == "req123" && t[2] == "wss://relay.com")
    );
    assert!(tags.iter().any(|t| t[0] == "p" && t[1] == "cust456"));
    assert!(tags.iter().any(|t| t[0] == "amount" && t[1] == "1000"));
}

// =========================================================================
// JobFeedback tests
// =========================================================================

#[test]
fn test_job_feedback_new() {
    let feedback = JobFeedback::new(JobStatus::Processing, "req123", "cust456");

    assert_eq!(feedback.status, JobStatus::Processing);
    assert_eq!(feedback.request_id, "req123");
    assert_eq!(feedback.customer_pubkey, "cust456");
}

#[test]
fn test_job_feedback_builder() {
    let feedback = JobFeedback::new(JobStatus::PaymentRequired, "req123", "cust456")
        .with_status_extra("Please pay to continue")
        .with_request_relay("wss://relay.com")
        .with_amount(5000, Some("lnbc...".to_string()));

    assert_eq!(
        feedback.status_extra,
        Some("Please pay to continue".to_string())
    );
    assert_eq!(feedback.request_relay, Some("wss://relay.com".to_string()));
    assert_eq!(feedback.amount, Some(5000));
    assert_eq!(feedback.bolt11, Some("lnbc...".to_string()));
}

#[test]
fn test_job_feedback_partial_with_content() {
    let feedback = JobFeedback::new(JobStatus::Partial, "req123", "cust456")
        .with_content("Here's a sample of the results...");

    assert_eq!(feedback.status, JobStatus::Partial);
    assert_eq!(feedback.content, "Here's a sample of the results...");
}

#[test]
fn test_job_feedback_to_tags() {
    let feedback = JobFeedback::new(JobStatus::Error, "req123", "cust456")
        .with_status_extra("Out of credits")
        .with_request_relay("wss://relay.com");

    let tags = feedback.to_tags();

    assert!(
        tags.iter()
            .any(|t| t[0] == "status" && t[1] == "error" && t[2] == "Out of credits")
    );
    assert!(tags.iter().any(|t| t[0] == "e" && t[1] == "req123"));
    assert!(tags.iter().any(|t| t[0] == "p" && t[1] == "cust456"));
}

// =========================================================================
// Integration tests - DVM workflow
// =========================================================================

#[test]
fn test_dvm_workflow_text_generation() {
    // 1. Customer creates a job request
    let request = JobRequest::new(KIND_JOB_TEXT_GENERATION)
        .unwrap()
        .add_input(JobInput::text("What is the capital of France?"))
        .add_param("model", "LLaMA-2")
        .add_param("max_tokens", "512")
        .with_bid(1000)
        .add_relay("wss://relay.damus.io");

    assert_eq!(request.kind, 5050);
    assert_eq!(request.result_kind(), 6050);

    // 2. Service provider sends processing feedback
    let feedback = JobFeedback::new(JobStatus::Processing, "req_event_id", "customer_pubkey");
    assert_eq!(feedback.status, JobStatus::Processing);

    // 3. Service provider sends result
    let result = JobResult::new(
        request.kind,
        "req_event_id",
        "customer_pubkey",
        "The capital of France is Paris.",
    )
    .unwrap()
    .with_amount(500, Some("lnbc500n1...".to_string()));

    assert_eq!(result.kind, 6050);
    assert_eq!(result.content, "The capital of France is Paris.");
    assert_eq!(result.amount, Some(500));
}

#[test]
fn test_dvm_workflow_job_chaining() {
    // Job 1: Speech to text
    let job1 = JobRequest::new(KIND_JOB_SPEECH_TO_TEXT)
        .unwrap()
        .add_input(JobInput::url("https://example.com/podcast.mp3"));

    // Job 2: Summarization (uses output of job 1)
    let job2 = JobRequest::new(KIND_JOB_SUMMARIZATION)
        .unwrap()
        .add_input(JobInput::job(
            "job1_event_id",
            Some("wss://relay.com".to_string()),
        ));

    assert_eq!(job1.kind, 5250);
    assert_eq!(job2.kind, 5001);
    assert_eq!(job2.inputs[0].input_type, InputType::Job);
}

#[test]
fn test_common_job_kinds() {
    // Verify common job kinds are in valid range
    assert!(is_job_request_kind(KIND_JOB_TEXT_EXTRACTION));
    assert!(is_job_request_kind(KIND_JOB_SUMMARIZATION));
    assert!(is_job_request_kind(KIND_JOB_TRANSLATION));
    assert!(is_job_request_kind(KIND_JOB_TEXT_GENERATION));
    assert!(is_job_request_kind(KIND_JOB_IMAGE_GENERATION));
    assert!(is_job_request_kind(KIND_JOB_SPEECH_TO_TEXT));

    // Verify corresponding result kinds
    assert_eq!(get_result_kind(KIND_JOB_TEXT_GENERATION), Some(6050));
    assert_eq!(get_result_kind(KIND_JOB_SPEECH_TO_TEXT), Some(6250));
}

// =========================================================================
// Event builder helper tests
// =========================================================================

#[test]
fn test_create_job_request_event() {
    let request = JobRequest::new(KIND_JOB_TEXT_GENERATION)
        .unwrap()
        .add_input(JobInput::text("Write a haiku"))
        .with_bid(1000);

    let event = create_job_request_event(&request);

    assert_eq!(event.kind, KIND_JOB_TEXT_GENERATION);
    assert!(event.tags.iter().any(|t| t[0] == "i"));
    assert!(event.tags.iter().any(|t| t[0] == "bid" && t[1] == "1000"));
    assert_eq!(event.content, "");
}

#[test]
fn test_create_job_result_event() {
    let result = JobResult::new(
        KIND_JOB_TEXT_GENERATION,
        "request123",
        "customer456",
        "Nostr flows free",
    )
    .unwrap()
    .with_amount(1000, Some("lnbc...".to_string()));

    let event = create_job_result_event(&result);

    assert_eq!(event.kind, KIND_JOB_TEXT_GENERATION + 1000);
    assert!(
        event
            .tags
            .iter()
            .any(|t| t[0] == "e" && t[1] == "request123")
    );
    assert!(
        event
            .tags
            .iter()
            .any(|t| t[0] == "p" && t[1] == "customer456")
    );
    assert!(
        event
            .tags
            .iter()
            .any(|t| t[0] == "amount" && t[1] == "1000")
    );
    assert_eq!(event.content, "Nostr flows free");
}

#[test]
fn test_create_job_feedback_event() {
    let feedback = JobFeedback::new(JobStatus::Processing, "request123", "customer456")
        .with_status_extra("Working on it");

    let event = create_job_feedback_event(&feedback);

    assert_eq!(event.kind, KIND_JOB_FEEDBACK);
    assert!(
        event
            .tags
            .iter()
            .any(|t| t[0] == "status" && t[1] == "processing")
    );
    assert!(
        event
            .tags
            .iter()
            .any(|t| t[0] == "e" && t[1] == "request123")
    );
    assert!(
        event
            .tags
            .iter()
            .any(|t| t[0] == "p" && t[1] == "customer456")
    );
    assert_eq!(event.content, "");
}
