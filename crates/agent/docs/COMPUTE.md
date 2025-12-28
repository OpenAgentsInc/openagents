# Compute Client

This document explains how agents discover compute providers and pay for inference.

## Overview

Sovereign agents are **customers** in the NIP-90 compute marketplace. They:

1. Discover providers via NIP-89 (kind:31990)
2. Join provider's NIP-28 channel
3. Send JobRequest
4. Receive Invoice
5. Pay with Spark wallet
6. Receive JobResult

The key insight: **The agent IS the customer.** It uses the same flow as `agent_customer.rs` but autonomously.

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
    pub channel_id: String,      // NIP-28 channel to join
    pub relay_url: String,
    pub price_msats: u64,
    pub models: Vec<String>,
}

// Parse from HandlerInfo
let handler = HandlerInfo::from_event(&event)?;

if handler.handler_type == HandlerType::ComputeProvider {
    let channel_id = handler.custom_tags
        .iter()
        .find(|(k, _)| k == "channel")
        .map(|(_, v)| v.clone());

    providers.push(ProviderInfo {
        pubkey: handler.pubkey,
        name: handler.metadata.name,
        channel_id,
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

## Requesting Inference

### Join Provider Channel

```rust
// Subscribe to provider's NIP-28 channel
let filters = vec![json!({
    "kinds": [42],  // KIND_CHANNEL_MESSAGE
    "#e": [channel_id]
})];

let rx = relay.subscribe_with_channel("agent-channel", &filters).await?;
```

### Send JobRequest

```rust
let request = AgentMessage::JobRequest {
    kind: 5050,  // KIND_JOB_TEXT_GENERATION
    prompt: prompt.to_string(),
    max_tokens: 500,
    target_provider: Some(provider.pubkey.clone()),
};

send_channel_message(&channel_id, &request).await?;
```

### Receive Invoice

```rust
match parse_agent_message(&event.content) {
    Some(AgentMessage::Invoice {
        bolt11,
        job_id,
        amount_msats,
        payment_hash,
    }) => {
        // Check amount within budget
        let amount_sats = amount_msats / 1000;
        if amount_sats > budget_sats {
            return Err(anyhow!("Invoice exceeds budget"));
        }

        // Pay the invoice
        let payment = wallet.send_payment_simple(&bolt11, None).await?;

        // Confirm payment
        let confirm = AgentMessage::PaymentSent {
            job_id,
            payment_id: payment.payment.id,
        };
        send_channel_message(&channel_id, &confirm).await?;
    }
    // ...
}
```

### Receive Result

```rust
Some(AgentMessage::JobResult { job_id, result }) => {
    if our_job_id.as_ref() == Some(&job_id) {
        return Ok(result);
    }
}

// Or streaming chunks
Some(AgentMessage::StreamChunk { job_id, chunk, is_final }) => {
    if our_job_id.as_ref() == Some(&job_id) {
        result_text.push_str(&chunk);
        if is_final {
            return Ok(result_text);
        }
    }
}
```

## Message Protocol

### JobRequest

```json
{
  "type": "JobRequest",
  "kind": 5050,
  "prompt": "Summarize the latest Bitcoin news",
  "max_tokens": 500,
  "target_provider": "pubkey..."
}
```

### Invoice

```json
{
  "type": "Invoice",
  "bolt11": "lnbc...",
  "job_id": "job-123",
  "amount_msats": 5000,
  "payment_hash": "abc..."
}
```

### PaymentSent

```json
{
  "type": "PaymentSent",
  "job_id": "job-123",
  "payment_id": "payment-456"
}
```

### JobResult

```json
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

    // Request inference and PAY for it
    let reasoning = self.compute_client
        .request_inference(provider, &prompt, 500, budget_sats)
        .await?;

    let compute_cost_sats = provider.price_msats / 1000;

    // Record spend
    state.record_spend(compute_cost_sats);

    // ... parse actions, execute ...
}
```

## HTLC Escrow (Advanced)

For trustless payments, agents can use HTLC escrow:

1. Agent generates preimage
2. Agent sends HTLC payment (locked until preimage revealed)
3. Provider delivers result
4. Agent releases preimage
5. Provider claims payment

This prevents providers from taking payment without delivering results.

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

// Request inference
let result = compute_client.request_inference(
    provider,
    "What is the capital of France?",
    100,  // max_tokens
    1000, // budget_sats
).await?;

println!("Result: {}", result);
```
