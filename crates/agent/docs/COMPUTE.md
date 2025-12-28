# Compute Client

This document explains how agents discover compute providers and pay for inference.

## Overview

Sovereign agents are **customers** in the NIP-90 compute marketplace. They:

1. Discover providers via NIP-89 (kind:31990)
2. Send job request via direct NIP-90 event (kind:5050)
3. Receive job feedback with invoice (kind:7000)
4. Pay Lightning invoice with Spark wallet
5. Receive job result (kind:6050)

The key insight: **The agent IS the customer.** It uses the same flow as `agent_customer.rs` but autonomously.

## Primary Flow: Direct NIP-90 Events

The primary flow uses direct Nostr events - no channel required:

```text
Customer                    Relay                       Provider
   |                          |                            |
   | -- kind:5050 request --> | --> kind:5050 request ---> |
   |                          |                            |
   | <-- kind:7000 feedback --|<--- kind:7000 + invoice ---|
   |                          |                            |
   | [pay Lightning invoice]  |                            |
   |                          |                            |
   | <-- kind:6050 result ----|<--- kind:6050 result ------|
```

### Event Kinds

| Kind | Name | Direction | Purpose |
|------|------|-----------|---------|
| 5050 | JobRequest | Customer → Provider | Request compute with prompt |
| 7000 | JobFeedback | Provider → Customer | Status updates, payment invoice |
| 6050 | JobResult | Provider → Customer | Completed inference result |

## Optional: NIP-28 Channel Coordination

For multi-party scenarios or real-time discussion, agents can optionally use NIP-28 channels.
This is **NOT required** for the core compute flow.

Use channels when you need:
- Multi-party coordination
- Real-time chat between agents
- HTLC escrow payments (experimental)

## Provider Discovery

Agents discover compute providers by querying NIP-89 handler info events:

```rust
// Query for handler info events
let filters = vec![json!({
    "kinds": [31990],  // KIND_HANDLER_INFO
    "limit": 50
})];

let mut rx = relay.subscribe_with_channel("provider-discovery", &filters).await?;
```

### Parsing Provider Info

```rust
#[derive(Debug, Clone)]
pub struct ProviderInfo {
    pub pubkey: String,
    pub name: String,
    /// NIP-28 channel ID (optional - only if provider uses channels)
    pub channel_id: Option<String>,
    pub relay_url: String,
    pub price_msats: u64,
    pub models: Vec<String>,
}

// Parse from HandlerInfo
let handler = HandlerInfo::from_event(&event)?;

if handler.handler_type == HandlerType::ComputeProvider {
    // Channel is optional
    let channel_id = handler.custom_tags
        .iter()
        .find(|(k, _)| k == "channel")
        .map(|(_, v)| v.clone());

    providers.push(ProviderInfo {
        pubkey: handler.pubkey,
        name: handler.metadata.name,
        channel_id,  // May be None for direct-events-only providers
        price_msats: handler.pricing.map(|p| p.amount).unwrap_or(0),
        // ...
    });
}
```

## Provider Selection

The compute client selects the cheapest provider within budget:

```rust
pub fn select_cheapest_provider(
    providers: &[ProviderInfo],
    budget_sats: u64,
) -> Option<&ProviderInfo> {
    providers
        .iter()
        .filter(|p| p.price_msats / 1000 <= budget_sats)
        .min_by_key(|p| p.price_msats)
}
```

## Requesting Inference (Direct NIP-90 Flow)

### Publish Job Request (kind:5050)

```rust
use openagents::agents::{publish_job_request, KIND_JOB_REQUEST_TEXT};

let job_request_id = publish_job_request(
    &relay,
    identity.keypair(),
    &provider.pubkey,
    prompt,
    max_tokens,
    KIND_JOB_REQUEST_TEXT,  // 5050
).await?;
```

### Subscribe to Job Responses

```rust
use openagents::agents::subscribe_job_responses;

let mut rx = subscribe_job_responses(&relay, &job_request_id).await?;
```

### Handle Feedback (kind:7000)

```rust
use openagents::agents::{parse_job_feedback, JobStatus, KIND_JOB_FEEDBACK};

if event.kind == KIND_JOB_FEEDBACK {
    if let Some((job_id, status, bolt11, amount)) = parse_job_feedback(&event) {
        match status {
            JobStatus::PaymentRequired => {
                let bolt11 = bolt11.expect("Invoice in feedback");
                let amount_sats = amount.unwrap_or(10_000) / 1000;

                // Pay the invoice
                wallet.send_payment_simple(&bolt11, None).await?;
            }
            JobStatus::Processing => {
                println!("Job is processing...");
            }
            JobStatus::Success => {
                println!("Job completed!");
            }
            JobStatus::Error => {
                return Err("Job failed");
            }
            JobStatus::Cancelled => {
                return Err("Job cancelled");
            }
        }
    }
}
```

### Receive Result (kind:6050)

```rust
use openagents::agents::{parse_job_result, KIND_JOB_RESULT_TEXT};

if event.kind == KIND_JOB_RESULT_TEXT {
    if let Some((job_id, result)) = parse_job_result(&event) {
        if job_id == job_request_id {
            return Ok(result);
        }
    }
}
```

## Legacy: Channel-Based Flow

For backward compatibility or HTLC escrow, the channel-based flow is still supported:

### Join Provider Channel

```rust
// Subscribe to provider's NIP-28 channel
let filters = vec![json!({
    "kinds": [42],  // KIND_CHANNEL_MESSAGE
    "#e": [channel_id]
})];

let rx = relay.subscribe_with_channel("agent-channel", &filters).await?;
```

### Channel Message Protocol

```json
// JobRequest
{
  "type": "JobRequest",
  "kind": 5050,
  "prompt": "Summarize the latest Bitcoin news",
  "max_tokens": 500,
  "target_provider": "pubkey..."
}

// Invoice
{
  "type": "Invoice",
  "bolt11": "lnbc...",
  "job_id": "job-123",
  "amount_msats": 5000,
  "payment_hash": "abc..."
}

// PaymentSent
{
  "type": "PaymentSent",
  "job_id": "job-123",
  "payment_id": "payment-456"
}

// JobResult
{
  "type": "JobResult",
  "job_id": "job-123",
  "result": "Here is a summary of the latest Bitcoin news..."
}
```

## Budget Enforcement

The agent enforces spending limits:

```rust
// Check if spend is allowed
if !state.budget.can_spend(estimated_cost) {
    return Err(BudgetExhausted);
}

// After compute purchase
state.budget.record_spend(actual_cost);
```

### Budget Configuration

```toml
[runway]
daily_limit_sats = 100000     # Max spend per day
per_tick_limit_sats = 1000    # Max spend per tick
```

## Integration with Tick Executor

```rust
// In tick executor
pub async fn execute_tick(&mut self, trigger: TickTrigger) -> Result<TickResult> {
    // ... gather observations, build prompt ...

    // Discover providers
    let providers = self.compute_client.discover_providers(3).await?;
    if providers.is_empty() {
        return Err(anyhow!("No compute providers available"));
    }

    // Select cheapest provider within budget
    let budget_sats = state.budget.as_ref()
        .map(|b| b.limits.per_tick_limit_sats)
        .unwrap_or(1000);

    let provider = ComputeClient::select_cheapest_provider(&providers, budget_sats)
        .ok_or_else(|| anyhow!("No provider within budget"))?;

    // Request inference and PAY for it (uses direct NIP-90 events)
    let reasoning = self.compute_client
        .request_inference(provider, &prompt, 500, budget_sats)
        .await?;

    let compute_cost_sats = provider.price_msats / 1000;

    // Record spend
    state.record_spend(compute_cost_sats);

    // ... parse actions, execute ...
}
```

## HTLC Escrow (Advanced, Channel Mode Only)

For trustless payments, agents can use HTLC escrow via channels:

1. Agent generates preimage
2. Agent sends HTLC payment (locked until preimage revealed)
3. Provider delivers result
4. Agent releases preimage
5. Provider claims payment

This prevents providers from taking payment without delivering results.

Note: HTLC mode requires channel-based flow.

## Error Handling

### No Providers Available

```rust
if providers.is_empty() {
    return Err(anyhow!("No compute providers available"));
}
```

Make sure providers are running and have published NIP-89 handler info.

### Provider Too Expensive

```rust
if provider.price_msats / 1000 > budget_sats {
    return Err(anyhow!("No provider within budget"));
}
```

Increase budget or wait for cheaper providers.

### Payment Failed

```rust
match wallet.send_payment_simple(&bolt11, None).await {
    Ok(payment) => { /* success */ }
    Err(e) => {
        // Insufficient funds, network error, etc.
        return Err(anyhow!("Payment failed: {}", e));
    }
}
```

Check wallet balance and network connectivity.

### Timeout

```rust
let timeout = Duration::from_secs(120);

loop {
    if job_start.elapsed() > timeout {
        return Err(anyhow!("Timeout waiting for compute result"));
    }
    // ...
}
```

Provider may be slow or offline.

## Programmatic Usage

```rust
use openagents::agents::ComputeClient;

let compute_client = ComputeClient::new(identity, relay, wallet);

// Discover providers
let providers = compute_client.discover_providers(3).await?;

// Select provider
let provider = ComputeClient::select_cheapest_provider(&providers, 1000)
    .ok_or_else(|| anyhow!("No provider"))?;

// Request inference (uses direct NIP-90 events or channel if provider requires it)
let result = compute_client.request_inference(
    provider,
    "What is the capital of France?",
    100,  // max_tokens
    1000, // budget_sats
).await?;

println!("Result: {}", result);
```

## CLI Usage

### Provider (Computer A)

```bash
# Primary: Direct NIP-90 events (no channel)
cargo run --bin agent-provider

# Optional: Create channel for coordination
cargo run --bin agent-provider -- --create-channel

# Optional: Join existing channel
cargo run --bin agent-provider -- --channel <CHANNEL_ID>
```

### Customer (Computer B)

```bash
# Primary: Direct NIP-90 events (discovers provider via NIP-89)
cargo run --bin agent-customer -- --prompt "What is Bitcoin?"

# Optional: Use specific channel
cargo run --bin agent-customer -- --channel <CHANNEL_ID> --prompt "..."

# HTLC escrow mode (requires channel)
cargo run --bin agent-customer -- --htlc --channel <CHANNEL_ID> --prompt "..."
```
