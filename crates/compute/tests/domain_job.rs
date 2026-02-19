//! Unit tests for Job domain module

use compute::domain::job::StoredJobInput;
use compute::domain::{Job, JobStatus};
use nostr::JobInput;
use std::collections::HashMap;

// Helper function to create a test job
fn create_test_job() -> Job {
    let inputs = vec![JobInput::text("Test input")];
    let params = HashMap::from([
        ("model".to_string(), "llama3.2".to_string()),
        ("temperature".to_string(), "0.7".to_string()),
    ]);

    Job::new(
        "job123".to_string(),
        "event456".to_string(),
        5050,
        "customer_pubkey".to_string(),
        inputs,
        params,
    )
}

// =========================================================================
// Job creation and initialization
// =========================================================================

#[test]
fn test_job_creation() {
    let job = create_test_job();

    assert_eq!(job.id, "job123");
    assert_eq!(job.request_event_id, "event456");
    assert_eq!(job.kind, 5050);
    assert_eq!(job.customer_pubkey, "customer_pubkey");
    assert_eq!(job.inputs.len(), 1);
    assert_eq!(job.params.len(), 2);
    assert_eq!(job.status, JobStatus::Pending);
    assert!(job.amount_msats.is_none());
    assert!(job.bolt11.is_none());
    assert!(job.completed_at.is_none());
    assert!(job.model.is_none());
}

#[test]
fn test_job_with_no_inputs() {
    let job = Job::new(
        "job1".to_string(),
        "event1".to_string(),
        5050,
        "customer1".to_string(),
        vec![],
        HashMap::new(),
    );

    assert_eq!(job.inputs.len(), 0);
    assert_eq!(job.params.len(), 0);
}

#[test]
fn test_job_with_multiple_inputs() {
    let inputs = vec![
        JobInput::text("First input"),
        JobInput::url("https://example.com/data"),
        JobInput::event("event123", Some("wss://relay.com".to_string())),
    ];

    let job = Job::new(
        "job1".to_string(),
        "event1".to_string(),
        5050,
        "customer1".to_string(),
        inputs,
        HashMap::new(),
    );

    assert_eq!(job.inputs.len(), 3);
    assert_eq!(job.inputs[0].input_type, "text");
    assert_eq!(job.inputs[1].input_type, "url");
    assert_eq!(job.inputs[2].input_type, "event");
}

// =========================================================================
// StoredJobInput conversion tests
// =========================================================================

#[test]
fn test_stored_job_input_from_job_input_text() {
    let input = JobInput::text("Hello world");
    let stored = StoredJobInput::from(&input);

    assert_eq!(stored.data, "Hello world");
    assert_eq!(stored.input_type, "text");
    assert!(stored.relay.is_none());
    assert!(stored.marker.is_none());
}

#[test]
fn test_stored_job_input_from_job_input_with_relay() {
    let input = JobInput::event("event123", Some("wss://relay.com".to_string()));
    let stored = StoredJobInput::from(&input);

    assert_eq!(stored.data, "event123");
    assert_eq!(stored.input_type, "event");
    assert_eq!(stored.relay, Some("wss://relay.com".to_string()));
}

#[test]
fn test_stored_job_input_from_job_input_with_marker() {
    let input = JobInput::text("data").with_marker("primary");
    let stored = StoredJobInput::from(&input);

    assert_eq!(stored.marker, Some("primary".to_string()));
}

#[test]
fn test_stored_job_input_to_job_input_roundtrip() {
    let original =
        JobInput::event("event123", Some("wss://relay.com".to_string())).with_marker("source");
    let stored = StoredJobInput::from(&original);
    let recovered = stored.to_job_input().unwrap();

    assert_eq!(recovered.data, original.data);
    assert_eq!(recovered.input_type, original.input_type);
    assert_eq!(recovered.relay, original.relay);
    assert_eq!(recovered.marker, original.marker);
}

// =========================================================================
// Job helper methods
// =========================================================================

#[test]
fn test_text_input_extraction() {
    let job = create_test_job();
    assert_eq!(job.text_input(), Some("Test input"));
}

#[test]
fn test_text_input_none() {
    let inputs = vec![JobInput::url("https://example.com")];
    let job = Job::new(
        "job1".to_string(),
        "event1".to_string(),
        5050,
        "customer1".to_string(),
        inputs,
        HashMap::new(),
    );

    assert!(job.text_input().is_none());
}

#[test]
fn test_text_input_multiple_finds_first() {
    let inputs = vec![JobInput::text("First text"), JobInput::text("Second text")];
    let job = Job::new(
        "job1".to_string(),
        "event1".to_string(),
        5050,
        "customer1".to_string(),
        inputs,
        HashMap::new(),
    );

    assert_eq!(job.text_input(), Some("First text"));
}

#[test]
fn test_requested_model() {
    let job = create_test_job();
    assert_eq!(job.requested_model(), Some("llama3.2"));
}

#[test]
fn test_requested_model_none() {
    let job = Job::new(
        "job1".to_string(),
        "event1".to_string(),
        5050,
        "customer1".to_string(),
        vec![],
        HashMap::new(),
    );

    assert!(job.requested_model().is_none());
}

// =========================================================================
// Job status transitions
// =========================================================================

#[test]
fn test_is_terminal_pending() {
    let job = create_test_job();
    assert!(!job.is_terminal());
}

#[test]
fn test_is_terminal_processing() {
    let mut job = create_test_job();
    job.set_processing();
    assert!(!job.is_terminal());
}

#[test]
fn test_is_terminal_completed() {
    let mut job = create_test_job();
    job.status = JobStatus::Completed {
        result: "Done".to_string(),
    };
    assert!(job.is_terminal());
}

#[test]
fn test_is_terminal_failed() {
    let mut job = create_test_job();
    job.status = JobStatus::Failed {
        error: "Error".to_string(),
    };
    assert!(job.is_terminal());
}

#[test]
fn test_set_processing() {
    let mut job = create_test_job();
    job.set_processing();

    assert_eq!(job.status, JobStatus::Processing { progress: None });
}

#[test]
fn test_set_progress() {
    let mut job = create_test_job();
    job.set_processing();
    job.set_progress(0.5);

    assert_eq!(
        job.status,
        JobStatus::Processing {
            progress: Some(0.5)
        }
    );
}

#[test]
fn test_set_progress_updates_existing() {
    let mut job = create_test_job();
    job.set_processing();
    job.set_progress(0.25);
    job.set_progress(0.75);

    assert_eq!(
        job.status,
        JobStatus::Processing {
            progress: Some(0.75)
        }
    );
}

// =========================================================================
// JobStatus variants
// =========================================================================

#[test]
fn test_job_status_pending() {
    let status = JobStatus::Pending;
    assert_eq!(status, JobStatus::Pending);
}

#[test]
fn test_job_status_payment_required() {
    let status = JobStatus::PaymentRequired {
        bolt11: "lnbc...".to_string(),
        amount_msats: 1000,
    };

    match status {
        JobStatus::PaymentRequired {
            bolt11,
            amount_msats,
        } => {
            assert_eq!(bolt11, "lnbc...");
            assert_eq!(amount_msats, 1000);
        }
        _ => panic!("Wrong status variant"),
    }
}

#[test]
fn test_job_status_processing_no_progress() {
    let status = JobStatus::Processing { progress: None };

    match status {
        JobStatus::Processing { progress } => {
            assert!(progress.is_none());
        }
        _ => panic!("Wrong status variant"),
    }
}

#[test]
fn test_job_status_processing_with_progress() {
    let status = JobStatus::Processing {
        progress: Some(0.42),
    };

    match status {
        JobStatus::Processing { progress } => {
            assert_eq!(progress, Some(0.42));
        }
        _ => panic!("Wrong status variant"),
    }
}

#[test]
fn test_job_status_completed() {
    let status = JobStatus::Completed {
        result: "Success!".to_string(),
    };

    match status {
        JobStatus::Completed { result } => {
            assert_eq!(result, "Success!");
        }
        _ => panic!("Wrong status variant"),
    }
}

#[test]
fn test_job_status_failed() {
    let status = JobStatus::Failed {
        error: "Model not found".to_string(),
    };

    match status {
        JobStatus::Failed { error } => {
            assert_eq!(error, "Model not found");
        }
        _ => panic!("Wrong status variant"),
    }
}

// =========================================================================
// Job parameter handling
// =========================================================================

#[test]
fn test_params_multiple_values() {
    let params = HashMap::from([
        ("model".to_string(), "llama3.2".to_string()),
        ("temperature".to_string(), "0.7".to_string()),
        ("max_tokens".to_string(), "2048".to_string()),
    ]);

    let job = Job::new(
        "job1".to_string(),
        "event1".to_string(),
        5050,
        "customer1".to_string(),
        vec![],
        params,
    );

    assert_eq!(job.params.get("model"), Some(&"llama3.2".to_string()));
    assert_eq!(job.params.get("temperature"), Some(&"0.7".to_string()));
    assert_eq!(job.params.get("max_tokens"), Some(&"2048".to_string()));
}

#[test]
fn test_params_get_nonexistent() {
    let job = create_test_job();
    assert!(!job.params.contains_key("nonexistent"));
}

// =========================================================================
// Job metadata
// =========================================================================

#[test]
fn test_job_timestamps() {
    let job = create_test_job();

    assert!(job.completed_at.is_none());

    // Created timestamp should be recent (within last second)
    let now = chrono::Utc::now();
    let diff = now.signed_duration_since(job.created_at);
    assert!(diff.num_seconds() < 1);
}

#[test]
fn test_job_kind_values() {
    let job1 = Job::new(
        "j1".to_string(),
        "e1".to_string(),
        5000,
        "c1".to_string(),
        vec![],
        HashMap::new(),
    );
    assert_eq!(job1.kind, 5000);

    let job2 = Job::new(
        "j2".to_string(),
        "e2".to_string(),
        5999,
        "c2".to_string(),
        vec![],
        HashMap::new(),
    );
    assert_eq!(job2.kind, 5999);
}

// =========================================================================
// Edge cases
// =========================================================================

#[test]
fn test_empty_job_id() {
    let job = Job::new(
        "".to_string(),
        "event1".to_string(),
        5050,
        "customer1".to_string(),
        vec![],
        HashMap::new(),
    );

    assert_eq!(job.id, "");
}

#[test]
fn test_empty_customer_pubkey() {
    let job = Job::new(
        "job1".to_string(),
        "event1".to_string(),
        5050,
        "".to_string(),
        vec![],
        HashMap::new(),
    );

    assert_eq!(job.customer_pubkey, "");
}

#[test]
fn test_very_long_input_data() {
    let long_data = "x".repeat(10000);
    let inputs = vec![JobInput::text(&long_data)];
    let job = Job::new(
        "job1".to_string(),
        "event1".to_string(),
        5050,
        "customer1".to_string(),
        inputs,
        HashMap::new(),
    );

    assert_eq!(job.inputs[0].data.len(), 10000);
}

#[test]
fn test_unicode_in_inputs() {
    let unicode_data = "こんにちは 世界 🌍";
    let inputs = vec![JobInput::text(unicode_data)];
    let job = Job::new(
        "job1".to_string(),
        "event1".to_string(),
        5050,
        "customer1".to_string(),
        inputs,
        HashMap::new(),
    );

    assert_eq!(job.text_input(), Some(unicode_data));
}

#[test]
fn test_special_characters_in_params() {
    let params = HashMap::from([
        ("key<>".to_string(), "value&\"'".to_string()),
        ("emoji".to_string(), "🔑🔐".to_string()),
    ]);

    let job = Job::new(
        "job1".to_string(),
        "event1".to_string(),
        5050,
        "customer1".to_string(),
        vec![],
        params,
    );

    assert_eq!(job.params.get("key<>"), Some(&"value&\"'".to_string()));
    assert_eq!(job.params.get("emoji"), Some(&"🔑🔐".to_string()));
}

// =========================================================================
// Serialization tests (verify Job is Serialize/Deserialize)
// =========================================================================

#[test]
fn test_job_serialization() {
    let job = create_test_job();
    let serialized = serde_json::to_string(&job).unwrap();
    assert!(serialized.contains("job123"));
    assert!(serialized.contains("event456"));
}

#[test]
fn test_job_deserialization() {
    let job = create_test_job();
    let serialized = serde_json::to_string(&job).unwrap();
    let deserialized: Job = serde_json::from_str(&serialized).unwrap();

    assert_eq!(deserialized.id, job.id);
    assert_eq!(deserialized.request_event_id, job.request_event_id);
    assert_eq!(deserialized.kind, job.kind);
}

#[test]
fn test_job_status_serialization() {
    let statuses = vec![
        JobStatus::Pending,
        JobStatus::PaymentRequired {
            bolt11: "lnbc...".to_string(),
            amount_msats: 1000,
        },
        JobStatus::Processing {
            progress: Some(0.5),
        },
        JobStatus::Completed {
            result: "Done".to_string(),
        },
        JobStatus::Failed {
            error: "Error".to_string(),
        },
    ];

    for status in statuses {
        let serialized = serde_json::to_string(&status).unwrap();
        let deserialized: JobStatus = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized, status);
    }
}
