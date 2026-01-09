# Provenance Tracking

This document explains the provenance system in the protocol crate, which provides full audit trails for job execution.

## Overview

Every job response includes provenance information that captures:

- **What model** was used
- **How** it was configured (sampling parameters)
- **What** went in and came out (hashes)
- **Who** executed it (provider identity)
- **When** it was executed

This enables:
- Reproducibility verification
- Quality auditing
- Cost tracking
- Dispute resolution

## Provenance Structure

```rust
pub struct Provenance {
    /// Model identifier used for inference
    pub model_id: String,

    /// Sampling parameters used
    pub sampling: SamplingParams,

    /// SHA-256 hash of the canonical JSON input
    pub input_sha256: Option<String>,

    /// SHA-256 hash of the canonical JSON output
    pub output_sha256: Option<String>,

    /// Nostr public key of the provider
    pub provider_pubkey: Option<String>,

    /// Unix timestamp when executed
    pub executed_at: Option<u64>,

    /// Duration in milliseconds
    pub duration_ms: Option<u64>,

    /// Token counts for billing
    pub tokens: Option<TokenCounts>,
}

pub struct SamplingParams {
    /// Temperature (0.0 = deterministic)
    pub temperature: f32,

    /// Top-p (nucleus) sampling
    pub top_p: Option<f32>,

    /// Top-k sampling
    pub top_k: Option<u32>,

    /// Random seed for reproducibility
    pub seed: Option<u64>,

    /// Maximum tokens to generate
    pub max_tokens: Option<u32>,

    /// Stop sequences
    pub stop: Vec<String>,
}

pub struct TokenCounts {
    /// Input/prompt tokens
    pub input: u32,

    /// Output/completion tokens
    pub output: u32,
}
```

## Creating Provenance

### Builder Pattern

```rust
use protocol::provenance::{Provenance, SamplingParams};

// Basic provenance
let provenance = Provenance::new("claude-3-sonnet");

// With full details
let provenance = Provenance::new("claude-3-sonnet")
    .with_sampling(SamplingParams::deterministic(42))
    .with_input_hash("abc123...")
    .with_output_hash("def456...")
    .with_provider("npub1...")
    .with_executed_at(1700000000)
    .with_duration(150)
    .with_tokens(500, 200);
```

### Sampling Presets

```rust
// Deterministic (temperature=0, fixed seed)
let sampling = SamplingParams::deterministic(42);

// Creative (temperature>0, top_p sampling)
let sampling = SamplingParams::creative(0.8);

// Custom
let sampling = SamplingParams {
    temperature: 0.3,
    top_p: Some(0.95),
    top_k: None,
    seed: Some(12345),
    max_tokens: Some(4096),
    stop: vec!["```".to_string()],
};
```

## Use Cases

### 1. Reproducibility Verification

When a job needs to be re-run for verification:

```rust
// Original execution
let original_provenance = response.provenance();

// Verification run with same params
let verification_request = request.clone();
let verification_response = provider.execute(&verification_request).await?;

// Compare outputs
if original_provenance.output_sha256 == verification_response.provenance().output_sha256 {
    // Results match - verified
} else {
    // Results differ - investigate
}
```

### 2. Quality Auditing

Track which models perform best:

```rust
struct QualityMetrics {
    model_id: String,
    avg_confidence: f32,
    avg_duration_ms: u64,
    total_jobs: u32,
}

fn update_metrics(metrics: &mut HashMap<String, QualityMetrics>, response: &ChunkAnalysisResponse) {
    let provenance = response.provenance();
    let entry = metrics.entry(provenance.model_id.clone()).or_default();

    entry.total_jobs += 1;
    // Update rolling averages...
}
```

### 3. Cost Tracking

Calculate costs based on token usage:

```rust
fn calculate_cost(provenance: &Provenance, pricing: &Pricing) -> f64 {
    if let Some(tokens) = &provenance.tokens {
        let input_cost = tokens.input as f64 * pricing.input_per_token;
        let output_cost = tokens.output as f64 * pricing.output_per_token;
        input_cost + output_cost
    } else {
        0.0
    }
}
```

### 4. Dispute Resolution

When results are contested:

```rust
fn investigate_dispute(
    request: &ChunkAnalysisRequest,
    response: &ChunkAnalysisResponse,
) -> DisputeReport {
    let provenance = response.provenance();

    // Verify input hash matches request
    let expected_input_hash = request.compute_hash().unwrap();
    let input_verified = provenance.input_sha256.as_ref() == Some(&expected_input_hash);

    // Check execution time is reasonable
    let duration_reasonable = provenance.duration_ms
        .map(|d| d < 60000) // Less than 60 seconds
        .unwrap_or(false);

    DisputeReport {
        input_verified,
        duration_reasonable,
        model_id: provenance.model_id.clone(),
        executed_at: provenance.executed_at,
    }
}
```

## Serialization

Provenance serializes cleanly, omitting optional fields:

```json
{
  "model_id": "claude-3-sonnet",
  "sampling": {
    "temperature": 0.0,
    "seed": 42
  },
  "input_sha256": "abc123...",
  "output_sha256": "def456...",
  "provider_pubkey": "npub1...",
  "executed_at": 1700000000,
  "duration_ms": 150,
  "tokens": {
    "input": 500,
    "output": 200
  }
}
```

Minimal provenance (only model_id):

```json
{
  "model_id": "claude-3-sonnet",
  "sampling": {
    "temperature": 0.0
  }
}
```

## Provider Implementation

Providers should capture provenance during execution:

```rust
async fn execute_job(
    request: &ChunkAnalysisRequest,
    model: &Model,
    provider_key: &Keys,
) -> Result<ChunkAnalysisResponse> {
    // Capture start time
    let start = std::time::Instant::now();
    let executed_at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    // Compute input hash
    let input_hash = request.compute_hash()?;

    // Execute inference
    let (result, token_counts) = model.generate(request).await?;

    // Compute output hash
    let output_hash = canonical_hash(&result)?;

    // Build response with provenance
    let response = ChunkAnalysisResponse {
        summary: result.summary,
        symbols: result.symbols,
        // ... other fields ...
        provenance: Provenance::new(&model.id)
            .with_sampling(model.sampling_params.clone())
            .with_input_hash(input_hash)
            .with_output_hash(output_hash)
            .with_provider(&provider_key.public_key().to_string())
            .with_executed_at(executed_at)
            .with_duration(start.elapsed().as_millis() as u64)
            .with_tokens(token_counts.input, token_counts.output),
    };

    Ok(response)
}
```

## Best Practices

### For Providers

1. **Always include model_id**: Essential for tracking
2. **Include sampling params**: Critical for reproducibility
3. **Hash inputs and outputs**: Enables verification
4. **Record accurate timestamps**: Supports auditing
5. **Track token counts**: Required for billing

### For Consumers

1. **Verify input hashes**: Ensure provider received correct request
2. **Store provenance**: Keep for auditing and disputes
3. **Monitor token usage**: Track costs
4. **Compare across providers**: Evaluate quality

### For Orchestrators

1. **Aggregate provenance**: Combine from multiple providers
2. **Track latency**: Monitor `duration_ms` for SLA
3. **Log all provenance**: Support dispute resolution
4. **Alert on anomalies**: Unusual token counts, durations

## Token Counting

For accurate billing, track both input and output tokens:

```rust
impl TokenCounts {
    pub fn new(input: u32, output: u32) -> Self {
        Self { input, output }
    }

    pub fn total(&self) -> u32 {
        self.input + self.output
    }
}

// Example pricing calculation
let tokens = provenance.tokens.unwrap();
let cost = (tokens.input as f64 * 0.001) + (tokens.output as f64 * 0.002);
```

## Integration with Nostr

Provenance links to Nostr identities:

```rust
// Provider signs response with their Nostr key
let provider_pubkey = keys.public_key().to_hex();

let provenance = Provenance::new("model")
    .with_provider(provider_pubkey);

// Consumer can verify:
// 1. Response was signed by this pubkey
// 2. Provenance claims same pubkey
// 3. Pubkey has good reputation
```
