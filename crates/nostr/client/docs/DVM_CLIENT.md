# DvmClient - NIP-90 Data Vending Machine Client

The `DvmClient` provides a high-level interface for submitting NIP-90 jobs to Data Vending Machines (DVMs) on the Nostr network.

## Overview

DVMs enable on-demand computation over Nostr: money in, data out. The `DvmClient` handles:

- Job submission (kind 5000-5999)
- Result awaiting (kind 6000-6999)
- Feedback subscriptions (kind 7000)
- NIP-42 authentication
- Multi-relay operations

## Protocol Flow

```
Customer (DvmClient)         Service Provider (DVM)
       |                            |
       |---- Job Request ---------> |
       |      (kind 5000-5999)      |
       |                            |
       |<--- Job Feedback --------- |  (optional)
       |      (kind 7000)           |
       |                            |
       |<--- Job Result ----------- |
       |      (kind 6000-6999)      |
       |                            |
       |---- Payment ------------>  |
       |   (bolt11 or zap)          |
```

## Usage

### Basic Usage

```rust
use nostr_client::dvm::{DvmClient, JobSubmission};
use nostr::{JobRequest, JobInput, KIND_JOB_TEXT_GENERATION};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create client with private key
    let private_key = [0u8; 32]; // Your private key
    let client = DvmClient::new(private_key)?;

    // Build job request
    let request = JobRequest::new(KIND_JOB_TEXT_GENERATION)?
        .add_input(JobInput::text("What is the capital of France?"))
        .add_param("model", "llama3")
        .with_bid(1000); // 1000 millisats

    // Submit to relays
    let submission = client.submit_job(
        request,
        &["wss://nexus.openagents.com"],
    ).await?;

    println!("Job submitted: {}", submission.event_id);

    // Wait for result (with timeout)
    let result = client.await_result(
        &submission.event_id,
        Duration::from_secs(60),
    ).await?;

    println!("Result: {}", result.content);
    Ok(())
}
```

### RLM Queries (Kind 5940)

For Recursive Language Model queries:

```rust
use nostr::{KIND_JOB_RLM_SUBQUERY, JobRequest, JobInput};

let request = JobRequest::new(KIND_JOB_RLM_SUBQUERY)?
    .add_input(JobInput::text("What is 2+2?"))
    .with_bid(1_000_000); // 1 sat

let submission = client.submit_job(request, &["wss://nexus.openagents.com"]).await?;
let result = client.await_result(&submission.event_id, Duration::from_secs(30)).await?;
```

### Subscribing to Feedback

To receive progress updates during job processing:

```rust
let mut feedback_rx = client.subscribe_to_feedback(&submission.event_id).await?;

tokio::spawn(async move {
    while let Some(feedback) = feedback_rx.recv().await {
        match feedback.feedback.status {
            JobStatus::Processing => println!("Processing..."),
            JobStatus::PaymentRequired => {
                if let Some(bolt11) = feedback.feedback.bolt11 {
                    println!("Payment required: {}", bolt11);
                }
            }
            JobStatus::Success => println!("Success!"),
            JobStatus::Error => println!("Error: {:?}", feedback.feedback.status_extra),
            _ => {}
        }
    }
});
```

### Provider Discovery

Find DVM providers for a specific job kind:

```rust
let providers = client.discover_providers(
    KIND_JOB_TEXT_GENERATION,
    &["wss://nexus.openagents.com", "wss://relay.damus.io"],
).await?;

for provider in providers {
    println!("Provider: {} ({:?})", provider.pubkey, provider.name);
    println!("  Supports: {:?}", provider.supported_kinds);
}
```

## API Reference

### `DvmClient::new(private_key: [u8; 32]) -> Result<Self>`

Create a new DVM client with the given private key.

### `DvmClient::submit_job(request: JobRequest, relays: &[&str]) -> Result<JobSubmission>`

Submit a job request to DVM providers. The job is published to all specified relays.

Returns a `JobSubmission` containing:
- `event_id` - The Nostr event ID of the published job
- `request` - The original job request
- `submitted_at` - Timestamp when submitted
- `relays` - Relays the job was published to

### `DvmClient::await_result(job_id: &str, timeout: Duration) -> Result<JobResult>`

Wait for a job result with timeout. Blocks until either:
- A job result is received
- The timeout expires
- An error status is received in feedback

**Important**: Result receivers are pre-created during `submit_job` to prevent race conditions where results arrive before `await_result` is called.

### `DvmClient::subscribe_to_feedback(job_id: &str) -> Result<Receiver<JobFeedbackEvent>>`

Subscribe to job feedback events. Returns a channel that will receive:
- Processing status updates
- Payment requests (bolt11 invoices)
- Error messages

### `DvmClient::cancel_job(job_id: &str) -> Result<()>`

Cancel a job subscription. Stops listening for results and feedback.

### `DvmClient::discover_providers(job_kind: u16, relays: &[&str]) -> Result<Vec<DvmProvider>>`

Discover DVM providers for a specific job kind by querying NIP-89 handler info events.

## Job Kinds

### Standard NIP-90 Kinds

| Request Kind | Result Kind | Description |
|-------------|-------------|-------------|
| 5000 | 6000 | Generic job |
| 5050 | 6050 | Text generation |
| 5100 | 6100 | Text-to-speech |
| 5250 | 6250 | Image generation |

### OpenAgents Extensions

| Request Kind | Result Kind | Description |
|-------------|-------------|-------------|
| 5940 | 6940 | RLM sub-query |
| 5941 | 6941 | RLM aggregation |

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│                      DvmClient                           │
├─────────────────────────────────────────────────────────┤
│  ┌─────────────┐  ┌─────────────────┐  ┌─────────────┐  │
│  │ RelayPool   │  │ pending_results │  │ active_jobs │  │
│  │             │  │                 │  │             │  │
│  │ Multi-relay │  │ Job → Sender    │  │ Job → SubID │  │
│  │ connections │  │ channels        │  │ mapping     │  │
│  └─────────────┘  └─────────────────┘  └─────────────┘  │
│                                                          │
│  ┌─────────────────┐  ┌────────────────────────────┐    │
│  │ result_receivers│  │ pending_feedback           │    │
│  │                 │  │                            │    │
│  │ Job → Receiver  │  │ Job → Feedback Sender      │    │
│  │ (pre-created)   │  │ channels                   │    │
│  └─────────────────┘  └────────────────────────────┘    │
└─────────────────────────────────────────────────────────┘
```

### Race Condition Prevention

The `DvmClient` uses a pre-registration pattern to prevent a race condition where results arrive before `await_result` is called:

1. During `subscribe_to_job_events` (called from `submit_job`):
   - Create result channel `(tx, rx)`
   - Insert `tx` into `pending_results` **before** subscribing
   - Store `rx` in `result_receivers`

2. When result event arrives:
   - Spawned task looks up sender in `pending_results`
   - Sends result to the channel

3. During `await_result`:
   - Retrieve pre-created receiver from `result_receivers`
   - Wait on the receiver with timeout

This ensures results are never lost, even if they arrive immediately after job submission.

## NIP-42 Authentication

The `DvmClient` automatically handles NIP-42 authentication with relays that require it (like Nexus):

1. Set auth key on the relay pool: `client.pool.set_auth_key(private_key)`
2. On AUTH challenge, the client signs and sends an AUTH event
3. After authentication, job submissions are accepted

## Error Handling

```rust
use nostr_client::error::ClientError;

match client.await_result(&job_id, timeout).await {
    Ok(result) => println!("Success: {}", result.content),
    Err(ClientError::Timeout(msg)) => println!("Timeout: {}", msg),
    Err(ClientError::PublishFailed(msg)) => println!("Publish failed: {}", msg),
    Err(ClientError::Internal(msg)) => println!("Internal error: {}", msg),
    Err(e) => println!("Other error: {:?}", e),
}
```

## Related Documentation

- [NIP-90 Data Vending Machines](https://github.com/nostr-protocol/nips/blob/master/90.md)
- [NIP-89 Recommended Application Handlers](https://github.com/nostr-protocol/nips/blob/master/89.md)
- [NIP-42 Authentication](https://github.com/nostr-protocol/nips/blob/master/42.md)
- [Pylon CLI - RLM Command](../../pylon/docs/CLI.md#pylon-rlm)
