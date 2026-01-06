# LM Router

The `lm-router` crate provides a unified interface for routing LLM requests to multiple backends with usage tracking.

## Overview

```rust
use std::sync::Arc;
use lm_router::{LmRouter, backends::{FmBridgeBackend, MockBackend}};

// Build a router with multiple backends
let router = LmRouter::builder()
    .add_backend(FmBridgeBackend::new())
    .add_backend(MockBackend::new().with_model("test-model"))
    .route_model("gpt-4", "fm-bridge")
    .default_backend("fm-bridge")
    .build();

// Make a completion request
let response = router.complete("gpt-4", "Hello, world!", 100).await?;
println!("Response: {}", response.text);
println!("Tokens: {}", response.usage.total_tokens);
```

## LmBackend Trait

All backends implement the `LmBackend` trait:

```rust
#[async_trait]
pub trait LmBackend: Send + Sync {
    /// Backend name for identification.
    fn name(&self) -> &str;

    /// List of models this backend supports.
    fn supported_models(&self) -> Vec<String>;

    /// Check if a specific model is supported.
    fn supports_model(&self, model: &str) -> bool {
        self.supported_models().contains(&model.to_string())
    }

    /// Complete a prompt.
    async fn complete(
        &self,
        model: &str,
        prompt: &str,
        max_tokens: usize,
    ) -> Result<LmResponse>;

    /// Health check.
    async fn health_check(&self) -> bool;
}
```

## LmResponse

Standardized response format:

```rust
pub struct LmResponse {
    /// The generated text.
    pub text: String,
    /// Token usage statistics.
    pub usage: LmUsage,
    /// Model that generated the response.
    pub model: String,
    /// Why generation stopped.
    pub finish_reason: String,
    /// Response latency in milliseconds.
    pub latency_ms: u64,
}
```

## LmUsage

Token and cost tracking:

```rust
pub struct LmUsage {
    pub prompt_tokens: usize,
    pub completion_tokens: usize,
    pub total_tokens: usize,
    pub cost_usd: Option<f64>,
    pub cost_sats: Option<u64>,  // For swarm pricing
}
```

## Backends

### FM Bridge Backend

Apple Foundation Models via the `fm-bridge` crate:

```rust
use lm_router::backends::FmBridgeBackend;

let backend = FmBridgeBackend::new();
// Or with custom client:
let backend = FmBridgeBackend::with_client(fm_client);
```

### Mock Backend

Deterministic responses for testing:

```rust
use lm_router::backends::MockBackend;

let backend = MockBackend::new()
    .with_name("my-mock")
    .with_model("test-model")
    .with_response("Fixed response")
    .with_usage(100, 50);  // prompt_tokens, completion_tokens
```

### Swarm Simulator

Simulates a distributed NIP-90 swarm for testing:

```rust
use lm_router::backends::{SwarmSimulator, SwarmSimConfig, LatencyDist};

let config = SwarmSimConfig {
    latency: LatencyDist::Normal { mean_ms: 500, std_ms: 200 },
    failure_rate: 0.1,      // 10% of requests fail
    timeout_rate: 0.05,     // 5% timeout
    quorum_size: 3,         // Require 3 providers to agree
    variance_in_results: false,
    response_template: "Response: {prompt}".to_string(),
    ..Default::default()
};

let backend = SwarmSimulator::new(config);
```

#### Latency Distributions

```rust
pub enum LatencyDist {
    /// Constant latency.
    Constant(u64),
    /// Normal distribution.
    Normal { mean_ms: u64, std_ms: u64 },
    /// Long-tail distribution (realistic swarm behavior).
    LongTail { median_ms: u64, p99_ms: u64 },
}
```

#### Quorum Simulation

The swarm simulator can test quorum scenarios:

```rust
let config = SwarmSimConfig {
    num_providers: 5,
    quorum_size: 3,  // 3-of-5 must agree
    variance_in_results: true,  // Test verification logic
    ..Default::default()
};
```

## Router Configuration

### Model Routing

Route specific models to specific backends:

```rust
let router = LmRouter::builder()
    .add_backend(backend1)
    .add_backend(backend2)
    .route_model("gpt-4", "backend-1")
    .route_model("claude-3", "backend-2")
    .build();
```

### Default Backend

Set a fallback for unrouted models:

```rust
let router = LmRouter::builder()
    .add_backend(primary)
    .add_backend(fallback)
    .default_backend("fallback")
    .build();
```

### Auto-Routing

If no explicit route is set, the router checks which backends support the model:

```rust
// backend.supports_model(model) is called for each backend
let response = router.complete("any-model", prompt, max_tokens).await?;
```

## Usage Tracking

The router automatically tracks usage across all requests:

```rust
// Make some requests...
router.complete("model-a", "prompt 1", 100).await?;
router.complete("model-b", "prompt 2", 100).await?;

// Get usage report
let report = router.usage_report();
println!("Total calls: {}", report.total_calls);
println!("Total tokens: {}", report.total_tokens);
println!("Total cost: ${:.4}", report.total_cost_usd);

// Per-model breakdown
for (model, usage) in &report.by_model {
    println!("{}: {} calls, {} tokens", model, usage.call_count, usage.total_tokens);
}

// Reset tracking
router.reset_usage();
```

## Health Checks

Check backend availability:

```rust
let health = router.health_check().await;
for (backend, healthy) in health {
    println!("{}: {}", backend, if healthy { "OK" } else { "DOWN" });
}
```

## Error Handling

```rust
use lm_router::Error;

match router.complete("unknown-model", "prompt", 100).await {
    Ok(response) => println!("{}", response.text),
    Err(Error::BackendNotFound(model)) => {
        println!("No backend found for model: {}", model);
    }
    Err(Error::BackendError(msg)) => {
        println!("Backend error: {}", msg);
    }
    Err(e) => println!("Other error: {}", e),
}
```

## Thread Safety

`LmRouter` is `Send + Sync` and can be shared across tasks:

```rust
let router = Arc::new(router);

let handles: Vec<_> = (0..10).map(|i| {
    let router = router.clone();
    tokio::spawn(async move {
        router.complete("model", &format!("Prompt {}", i), 100).await
    })
}).collect();

for handle in handles {
    let _ = handle.await?;
}
```
