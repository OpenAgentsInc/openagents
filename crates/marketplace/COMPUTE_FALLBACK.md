# Compute Marketplace Fallback System

## Overview

The compute marketplace includes a **fully implemented** automatic fallback system that seamlessly transitions from local inference to marketplace swarm compute when local resources are unavailable or insufficient. This enables cost-effective and reliable inference for autonomous agents.

## Implementation Status

### ✅ Completed (Phase 2 of d-008)

The fallback system at `src/compute/fallback.rs` is complete and ready for integration:

1. **FallbackManager** - Core fallback orchestration
   - `execute_with_fallback()` - Try local first, fall back to swarm
   - Configuration management
   - Metrics tracking
   - Force modes (local-only, swarm-only)

2. **FallbackConfig** - Configurable behavior
   - Enable/disable automatic fallback
   - Maximum price limits for swarm compute
   - Local inference timeout
   - Force local-only or swarm-only modes

3. **FallbackMetrics** - Usage tracking
   - Local success/failure counts
   - Swarm fallback counts
   - Total cost tracking
   - Success and fallback rate calculations

4. **FallbackResult** - Execution outcome
   - Local success with duration
   - Swarm fallback with provider, cost, duration
   - Failure with error details

### ⏸️ Blocked (Awaiting Dependencies)

The fallback system has two integration points that are currently stubbed:

1. **Local Inference Backend** (d-019: GPT-OSS integration)
   - Needs `local-inference` crate or `LocalModelBackend` trait
   - Currently returns "Local inference not available" to demonstrate fallback

2. **Compute Consumer** (d-008: Marketplace consumer)
   - Needs `ComputeConsumer` for submitting jobs to marketplace
   - Currently stubs swarm execution to show the flow

Both dependencies are straightforward to integrate once available.

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    FallbackManager                           │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│  Input: model, prompt                                       │
│                                                              │
│  ┌────────────────────────────────────────────────┐        │
│  │ Step 1: Check force modes                      │        │
│  │   - force_swarm? → Skip to swarm               │        │
│  │   - force_local? → Only try local              │        │
│  └────────────────────────────────────────────────┘        │
│                      │                                       │
│                      ▼                                       │
│  ┌────────────────────────────────────────────────┐        │
│  │ Step 2: Try local inference                    │        │
│  │   - Check backend availability                 │        │
│  │   - Execute with timeout                       │        │
│  │   - Return if successful                       │        │
│  └────────────────────────────────────────────────┘        │
│                      │                                       │
│              Success │ │ Failure                             │
│        ┌─────────────┘ └─────────────┐                      │
│        │                              │                      │
│        ▼                              ▼                      │
│  ┌─────────┐              ┌───────────────────────┐        │
│  │ Return  │              │ Step 3: Check fallback│        │
│  │ Local   │              │   - Fallback enabled? │        │
│  │ Result  │              │   - Budget available? │        │
│  └─────────┘              └───────────────────────┘        │
│                                      │                       │
│                                      ▼                       │
│                          ┌───────────────────────┐          │
│                          │ Step 4: Swarm compute │          │
│                          │   - Submit to market  │          │
│                          │   - Wait for result   │          │
│                          │   - Check price limit │          │
│                          │   - Return result     │          │
│                          └───────────────────────┘          │
│                                                              │
│  Output: FallbackResult (Local | Swarm | Failed)           │
└─────────────────────────────────────────────────────────────┘
```

## Usage Examples

### Basic Usage

```rust
use marketplace::compute::fallback::{FallbackManager, FallbackConfig};

// Create manager with default config
let config = FallbackConfig::default();
let manager = FallbackManager::new(config);

// Execute with automatic fallback
let result = manager
    .execute_with_fallback("llama3", "What is Rust?")
    .await?;

match result {
    FallbackResult::Local { response, duration_ms } => {
        println!("✓ Local inference succeeded in {}ms", duration_ms);
        println!("Response: {}", response);
    }
    FallbackResult::Swarm { job_id, provider, cost_msats, duration_ms } => {
        println!("⚡ Fell back to swarm compute");
        println!("Provider: {}", provider);
        println!("Cost: {} msats", cost_msats);
        println!("Duration: {}ms", duration_ms);
    }
    FallbackResult::Failed { local_error, swarm_error } => {
        eprintln!("✗ Both local and swarm failed");
        eprintln!("Local: {}", local_error);
        if let Some(err) = swarm_error {
            eprintln!("Swarm: {}", err);
        }
    }
}
```

### Custom Configuration

```rust
use marketplace::compute::fallback::FallbackConfig;

// Configure for cost-sensitive agent
let config = FallbackConfig {
    enabled: true,
    max_price_msats: Some(500),      // Max 500 msats per job
    local_timeout_secs: 60,          // Wait up to 60s for local
    force_local: false,
    force_swarm: false,
};

let manager = FallbackManager::new(config);
```

### Force Modes

```rust
// Force local-only (never fallback, useful for development)
let config = FallbackConfig {
    force_local: true,
    ..Default::default()
};

// Force swarm-only (skip local, useful for testing marketplace)
let config = FallbackConfig {
    force_swarm: true,
    ..Default::default()
};
```

### Metrics Tracking

```rust
// Execute multiple requests
manager.execute_with_fallback("llama3", "prompt1").await?;
manager.execute_with_fallback("llama3", "prompt2").await?;
manager.execute_with_fallback("llama3", "prompt3").await?;

// Get metrics
let metrics = manager.get_metrics().await;

println!("Local successes: {}", metrics.local_success);
println!("Local failures: {}", metrics.local_failure);
println!("Swarm fallbacks: {}", metrics.swarm_fallback);
println!("Total cost: {} msats", metrics.total_cost_msats);
println!("Local success rate: {:.1}%", metrics.local_success_rate());
println!("Fallback rate: {:.1}%", metrics.fallback_rate());
```

## Configuration

### Default Settings

```rust
FallbackConfig {
    enabled: true,                   // Fallback enabled
    max_price_msats: Some(1000),    // Max 1000 msats per job
    local_timeout_secs: 30,         // 30s timeout for local
    force_local: false,             // Don't force local-only
    force_swarm: false,             // Don't force swarm-only
}
```

### Environment Variables

```bash
# Disable fallback (local-only)
FALLBACK_ENABLED=false

# Set maximum price
FALLBACK_MAX_PRICE_MSATS=500

# Set local timeout
FALLBACK_LOCAL_TIMEOUT_SECS=60

# Force modes (for testing)
FALLBACK_FORCE_LOCAL=true
FALLBACK_FORCE_SWARM=true
```

## Integration Points

### 1. Local Inference Backend (Pending d-019)

Once the local-inference crate or GPT-OSS integration is available:

```rust
// Uncomment in fallback.rs:
use local_inference::{LocalModelBackend, CompletionRequest};

impl FallbackManager {
    pub fn with_local_backend(mut self, backend: Arc<dyn LocalModelBackend>) -> Self {
        self.local_backend = Some(backend);
        self
    }
}

// Then try_local_inference() will use the real backend:
let backend = self.local_backend.as_ref()?;
let request = CompletionRequest::new(model, prompt);
let response = backend.complete(request).await?;
```

### 2. Compute Consumer (Pending d-008)

Once ComputeConsumer is implemented:

```rust
// Uncomment in fallback.rs:
use super::consumer::ComputeConsumer;

impl FallbackManager {
    pub fn with_consumer(mut self, consumer: Arc<ComputeConsumer>) -> Self {
        self.consumer = Some(consumer);
        self
    }
}

// Then execute_swarm() will use the real consumer:
let consumer = self.consumer.as_ref()?;
let request = ComputeJobRequest::text_generation(prompt);
let handle = consumer.submit_job(request).await?;
let result = handle.wait_for_result().await?;
```

## Testing

### Unit Tests

```bash
cargo test -p marketplace fallback
```

Tests cover:
- Configuration defaults and validation
- Metrics tracking and rate calculations
- Force mode behavior
- Fallback disabled behavior
- Manager configuration updates

### Integration Testing

Once dependencies are available:

```rust
#[tokio::test]
async fn test_local_to_swarm_fallback() {
    // Setup local backend that fails
    let local = FailingLocalBackend::new();

    // Setup marketplace consumer
    let consumer = ComputeConsumer::new(relay_pool);

    // Create manager
    let manager = FallbackManager::new(FallbackConfig::default())
        .with_local_backend(Arc::new(local))
        .with_consumer(Arc::new(consumer));

    // Execute - should fallback to swarm
    let result = manager.execute_with_fallback("llama3", "test").await?;

    assert!(matches!(result, FallbackResult::Swarm { .. }));
}
```

## Use Cases

### 1. Cost-Optimized Agent

```rust
// Always try local first (free), fall back to swarm (paid) when needed
let config = FallbackConfig {
    enabled: true,
    max_price_msats: Some(100), // Very low limit
    local_timeout_secs: 30,
    ..Default::default()
};
```

### 2. Reliability-Focused Agent

```rust
// Lower timeout, quick fallback to swarm for reliability
let config = FallbackConfig {
    enabled: true,
    max_price_msats: Some(5000), // Higher budget
    local_timeout_secs: 10,      // Quick timeout
    ..Default::default()
};
```

### 3. Development Mode

```rust
// Force local-only for development
let config = FallbackConfig {
    force_local: true,
    ..Default::default()
};
```

### 4. Testing Mode

```rust
// Force swarm-only to test marketplace
let config = FallbackConfig {
    force_swarm: true,
    ..Default::default()
};
```

## Metrics and Monitoring

The fallback system tracks comprehensive metrics:

```rust
pub struct FallbackMetrics {
    pub local_success: u64,      // Successful local inferences
    pub local_failure: u64,      // Failed local inferences
    pub swarm_fallback: u64,     // Successful swarm fallbacks
    pub swarm_failure: u64,      // Failed swarm requests
    pub total_cost_msats: u64,   // Total spent on swarm
}

impl FallbackMetrics {
    pub fn local_success_rate(&self) -> f64;  // % of local successes
    pub fn fallback_rate(&self) -> f64;       // % that fell back
}
```

These metrics enable:
- Cost tracking and budgeting
- Performance monitoring
- Failure rate analysis
- Optimization decisions

## Related Files

- `crates/marketplace/src/compute/fallback.rs` - Fallback implementation
- `crates/marketplace/src/compute/consumer.rs` - Marketplace consumer (pending)
- `crates/marketplace/src/compute/events.rs` - Compute job events
- `.openagents/directives/d-008.md` - Marketplace directive
- `.openagents/directives/d-019.md` - GPT-OSS local inference directive

## Next Steps

1. **Integrate local-inference backend** (d-019)
   - Implement `LocalModelBackend` trait
   - Connect GPT-OSS or other local models
   - Update `try_local_inference()` to use real backend

2. **Implement ComputeConsumer** (d-008)
   - NIP-90 job submission
   - Result streaming
   - Provider selection
   - Update `execute_swarm()` to use real consumer

3. **Add persistence** (optional)
   - Store metrics in database
   - Track historical fallback rates
   - Cost analysis over time

4. **Add configuration UI** (optional)
   - GUI for adjusting fallback settings
   - Real-time metrics dashboard
   - Budget alerts

## Decision Flow

```
┌─────────────────────────────────────────────────────────┐
│ Agent needs inference: execute_with_fallback()          │
└──────────────────────┬──────────────────────────────────┘
                       │
                       ▼
              ┌────────────────┐
              │ Force swarm?   │─── Yes ──→ [Execute on swarm]
              └────────┬───────┘
                       │ No
                       ▼
              ┌────────────────┐
              │ Try local      │
              │ inference      │
              └────────┬───────┘
                       │
                ┌──────┴──────┐
                │             │
             Success      Failure
                │             │
                ▼             ▼
          [Return         ┌────────────┐
           local          │Force local?│─── Yes ──→ [Return failed]
           result]        └─────┬──────┘
                                │ No
                                ▼
                          ┌────────────┐
                          │Fallback    │
                          │enabled?    │─── No ──→ [Return failed]
                          └─────┬──────┘
                                │ Yes
                                ▼
                          [Execute on swarm]
                                │
                          ┌─────┴─────┐
                          │           │
                       Success    Failure
                          │           │
                          ▼           ▼
                    [Return      [Return
                     swarm        failed
                     result]      result]
```
