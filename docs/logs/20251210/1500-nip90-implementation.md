# NIP-90 Implementation Log

**Date:** 2025-12-10
**Crate:** `crates/nostr`

## Summary

Implemented NIP-90 (Data Vending Machine) for the nostr crate.

## NIP-90 Specification

NIP-90 defines the interaction between customers and Service Providers for on-demand computation. "Money in, data out."

### Kind Ranges
- 5000-5999: Job request kinds
- 6000-6999: Job result kinds (request kind + 1000)
- 7000: Job feedback

### Protocol Flow
1. Customer publishes job request (kind 5000-5999)
2. Service Providers MAY send job feedback (kind 7000)
3. Service Provider publishes job result (kind 6000-6999)
4. Customer pays via bolt11 or zap

### Job Feedback Statuses
- `payment-required` - Payment needed before continuing
- `processing` - Job is being processed
- `error` - Processing failed
- `success` - Job completed successfully
- `partial` - Partial results available

## Implementation

### Public API

```rust
// Types
pub enum InputType { Url, Event, Job, Text }
pub enum JobStatus { PaymentRequired, Processing, Error, Success, Partial }
pub struct JobInput { data, input_type, relay, marker }
pub struct JobParam { key, value }
pub struct JobRequest { kind, inputs, output, params, bid, relays, ... }
pub struct JobResult { kind, content, request, request_id, customer_pubkey, amount, ... }
pub struct JobFeedback { status, status_extra, request_id, customer_pubkey, amount, ... }

// Kind constants
pub const KIND_JOB_TEXT_EXTRACTION: u16 = 5000;
pub const KIND_JOB_SUMMARIZATION: u16 = 5001;
pub const KIND_JOB_TRANSLATION: u16 = 5002;
pub const KIND_JOB_TEXT_GENERATION: u16 = 5050;
pub const KIND_JOB_IMAGE_GENERATION: u16 = 5100;
pub const KIND_JOB_SPEECH_TO_TEXT: u16 = 5250;
pub const KIND_JOB_FEEDBACK: u16 = 7000;

// Kind validation
pub fn is_job_request_kind(kind: u16) -> bool;
pub fn is_job_result_kind(kind: u16) -> bool;
pub fn is_job_feedback_kind(kind: u16) -> bool;
pub fn is_dvm_kind(kind: u16) -> bool;
pub fn get_result_kind(request_kind: u16) -> Option<u16>;
pub fn get_request_kind(result_kind: u16) -> Option<u16>;
```

### Builder Pattern Examples

```rust
// Create a job request
let request = JobRequest::new(KIND_JOB_TEXT_GENERATION)?
    .add_input(JobInput::text("What is the capital of France?"))
    .add_param("model", "LLaMA-2")
    .add_param("max_tokens", "512")
    .with_bid(1000)
    .add_relay("wss://relay.damus.io");

// Create a job result
let result = JobResult::new(5050, "req_id", "customer_pubkey", "Paris is the capital.")?
    .with_amount(500, Some("lnbc500n1...".to_string()));

// Create job feedback
let feedback = JobFeedback::new(JobStatus::Processing, "req_id", "customer_pubkey")
    .with_status_extra("Estimated completion: 30 seconds");
```

### Test Coverage (35 NIP-90 tests)

#### Kind Validation Tests
- `test_is_job_request_kind` - 5000-5999 range
- `test_is_job_result_kind` - 6000-6999 range
- `test_is_job_feedback_kind` - 7000
- `test_is_dvm_kind` - All DVM kinds
- `test_get_result_kind` - Request → Result mapping
- `test_get_request_kind` - Result → Request mapping

#### InputType Tests
- `test_input_type_as_str` - Enum to string
- `test_input_type_from_str` - String to enum (case insensitive)

#### JobStatus Tests
- `test_job_status_as_str` - Enum to string
- `test_job_status_from_str` - String to enum

#### JobInput Tests
- `test_job_input_text` - Text input creation
- `test_job_input_url` - URL input creation
- `test_job_input_event` - Event input with relay
- `test_job_input_job` - Job chaining input
- `test_job_input_with_marker` - Marker field
- `test_job_input_to_tag` - Convert to tag array
- `test_job_input_from_tag` - Parse from tag array

#### JobParam Tests
- `test_job_param` - Parameter creation
- `test_job_param_to_tag` - Convert to tag
- `test_job_param_from_tag` - Parse from tag

#### JobRequest Tests
- `test_job_request_new` - Basic creation
- `test_job_request_invalid_kind` - Kind validation
- `test_job_request_builder` - Full builder pattern
- `test_job_request_result_kind` - Result kind calculation
- `test_job_request_to_tags` - Tag generation

#### JobResult Tests
- `test_job_result_new` - Basic creation
- `test_job_result_builder` - Full builder pattern
- `test_job_result_to_tags` - Tag generation

#### JobFeedback Tests
- `test_job_feedback_new` - Basic creation
- `test_job_feedback_builder` - Full builder pattern
- `test_job_feedback_partial_with_content` - Partial results
- `test_job_feedback_to_tags` - Tag generation

#### Integration Tests
- `test_dvm_workflow_text_generation` - Full customer→SP flow
- `test_dvm_workflow_job_chaining` - Job output as input
- `test_common_job_kinds` - Verify common kinds

## Files

- `crates/nostr/src/lib.rs` - Added NIP-90 exports
- `crates/nostr/src/nip90.rs` - Implementation and tests (new file)

## Test Results

```
running 85 tests (25 NIP-01 + 25 NIP-06 + 35 NIP-90)
test result: ok. 85 passed; 0 failed
```

## Notes

- nostr-tools only has kind constants for DVM (JobRequest=5999, JobResult=6999, JobFeedback=7000)
- No specific NIP-90 implementation in nostr-tools, so created comprehensive implementation
- Builder pattern for ergonomic API
- Tag serialization/parsing for integration with NIP-01 events
- Support for job chaining (job output as input to another job)
- Support for encrypted params (flag only, encryption handled separately)
