# Compute Fallback: Local → Swarm

## Overview

The marketplace compute system supports automatic fallback from local inference to remote marketplace providers. This enables seamless transitions when local resources are unavailable or overloaded.

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    User Request                              │
│                         │                                    │
│                         ▼                                    │
│              ┌─────────────────────┐                         │
│              │  FallbackManager    │                         │
│              └─────────────────────┘                         │
│                         │                                    │
│           ┌─────────────┴─────────────┐                     │
│           ▼                           ▼                      │
│  ┌──────────────────┐      ┌──────────────────┐            │
│  │ Local Inference  │      │ Swarm Marketplace│            │
│  │  (if available)  │      │   (NIP-90 DVM)   │            │
│  └──────────────────┘      └──────────────────┘            │
│           │                           │                      │
│           └─────────────┬─────────────┘                     │
│                         ▼                                    │
│                    Response                                  │
└─────────────────────────────────────────────────────────────┘
```

## Usage

### CLI with `--local-first` Flag

```bash
# Try local inference first, fallback to swarm if unavailable
cargo marketplace compute submit \
    --job-type text-generation \
    --prompt "Explain Rust async" \
    --model llama3 \
    --local-first

# Standard marketplace submission (no local attempt)
cargo marketplace compute submit \
    --job-type text-generation \
    --prompt "Explain Rust async" \
    --model llama3
```

### Programmatic Usage

```rust
use marketplace::compute::fallback::{FallbackManager, FallbackConfig};

// Create fallback manager with configuration
let config = FallbackConfig {
    enabled: true,
    max_price_msats: Some(1000),      // Max 1000 msats per job
    local_timeout_secs: 30,           // 30s timeout for local
    force_local: false,               // Allow fallback
    force_swarm: false,               // Try local first
};

let manager = FallbackManager::new(config);

// Execute with automatic fallback
let result = manager.execute_with_fallback("llama3", "Hello world").await?;

match result {
    FallbackResult::Local { response, duration_ms } => {
        println!("Local: {} ({}ms)", response, duration_ms);
    }
    FallbackResult::Swarm { job_id, provider, cost_msats, duration_ms } => {
        println!("Swarm: job={}, provider={}, cost={}msats ({}ms)",
                 job_id, provider, cost_msats, duration_ms);
    }
    FallbackResult::Failed { local_error, swarm_error } => {
        eprintln!("Both failed! Local: {} Swarm: {:?}",
                  local_error, swarm_error);
    }
}
```

## Configuration Options

### FallbackConfig

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `enabled` | `bool` | `true` | Enable automatic fallback to marketplace |
| `max_price_msats` | `Option<u64>` | `Some(1000)` | Maximum price willing to pay for swarm |
| `local_timeout_secs` | `u64` | `30` | Timeout for local inference before fallback |
| `force_local` | `bool` | `false` | Never fallback (local-only mode) |
| `force_swarm` | `bool` | `false` | Skip local entirely (swarm-only mode) |

### Modes

#### 1. Standard Fallback (Default)
```rust
FallbackConfig {
    enabled: true,
    force_local: false,
    force_swarm: false,
    ..Default::default()
}
```
- Try local first
- Fallback to swarm if local fails/unavailable
- Best for general use

#### 2. Local-Only Mode
```rust
FallbackConfig {
    force_local: true,
    ..Default::default()
}
```
- Never use swarm
- Fail if local unavailable
- Best for privacy/cost-sensitive scenarios

#### 3. Swarm-Only Mode
```rust
FallbackConfig {
    force_swarm: true,
    ..Default::default()
}
```
- Skip local entirely
- Always use marketplace
- Best when local resources shouldn't be used

#### 4. Fallback Disabled
```rust
FallbackConfig {
    enabled: false,
    ..Default::default()
}
```
- Try local once
- Don't fallback even if local fails
- Best for testing local models

## Metrics

The FallbackManager tracks usage metrics:

```rust
let metrics = manager.get_metrics().await;

println!("Local success: {}", metrics.local_success);
println!("Local failure: {}", metrics.local_failure);
println!("Swarm fallback: {}", metrics.swarm_fallback);
println!("Swarm failure: {}", metrics.swarm_failure);
println!("Total cost: {} msats", metrics.total_cost_msats);

println!("Local success rate: {:.1}%", metrics.local_success_rate());
println!("Fallback rate: {:.1}%", metrics.fallback_rate());
```

## Database Tracking

Jobs that attempted local inference are tracked with the `local_attempted` flag:

```sql
SELECT job_id, state, local_attempted, provider
FROM jobs
WHERE local_attempted = 1;
```

- `local_attempted = 1`: Tried local first (regardless of success)
- `local_attempted = 0`: Went straight to marketplace

## Integration Status

### Current Implementation

✅ FallbackManager with full configuration
✅ Local → Swarm fallback logic
✅ CLI `--local-first` flag
✅ Metrics tracking
✅ Database schema with `local_attempted` flag
✅ Comprehensive test coverage

### Pending Integration

⏸️ Local inference backend connection (requires local-inference crate)
⏸️ Marketplace consumer connection (ComputeConsumer implementation)

When these integrations are complete, the fallback system will work end-to-end. Currently:
- Local inference returns "not available" (triggers fallback)
- Swarm submission returns stub job (demonstrates flow)

## Example Output

### Local Success
```
✓ Completed locally in 1234ms

The response from local model...
```

### Swarm Fallback
```
⚠ Local inference failed: No local backend configured - falling back to marketplace
✓ Submitted to swarm (local unavailable)
Job ID: job-abc123
Provider: provider-xyz
Cost: 500 msats (0 sats)
Duration: 2345ms
```

### Both Failed
```
✗ Job failed
Local error: No local inference backend configured
Swarm error: Price 2000 msats exceeds max 1000 msats
```

## See Also

- `fallback.rs` - Core fallback implementation
- `consumer.rs` - Marketplace job submission
- `../cli/compute.rs` - CLI integration
- d-008 directive - Marketplace architecture
