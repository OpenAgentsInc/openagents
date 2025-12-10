# Foundation Model API Bridge - Rust Implementation Plan

> **Created:** 2025-12-09
> **Priority:** P0 - First Rust crate to bootstrap
> **Purpose:** Complete Rust implementation of the Foundation Model (Apple Intelligence) API bridge
> **Dependencies:** None (standalone crate)

---

## Table of Contents

1. [Overview](#overview)
2. [Current Implementation Analysis](#current-implementation-analysis)
3. [Rust Crate Architecture](#rust-crate-architecture)
4. [API Surface](#api-surface)
5. [Implementation Plan](#implementation-plan)
6. [Testing Strategy](#testing-strategy)
7. [Performance Targets](#performance-targets)

---

## Overview

The Foundation Model API bridge provides access to Apple's on-device Foundation Models (Apple Intelligence) through a Swift → HTTP → Client architecture. This is critical for:

- **Terminal-Bench**: Using local FM for test generation and hillclimber
- **MechaCoder**: Running agents with on-device inference
- **Cost Savings**: $0 inference vs cloud API costs
- **Privacy**: All data stays on device
- **Performance**: Low latency local inference

**Current Status:**
- Swift bridge implementation: `swift-bridge/` (keep as-is)
- TypeScript client: `src/llm/foundation-models.ts`
- HTTP server: Running on `localhost:3030`

**Goal:**
Create a complete Rust crate `fm-bridge` that:
1. Replaces the TypeScript client
2. Provides ergonomic Rust API
3. Includes comprehensive tests
4. Supports all FM capabilities (prompts, streaming, tools, etc.)

---

## Current Implementation Analysis

### Swift Bridge (Keep As-Is)

Located in `swift-bridge/`, exposes Apple Foundation Models via HTTP:

**Endpoints:**
- `POST /v1/completions` - Non-streaming completion
- `POST /v1/completions/stream` - Streaming completion
- `POST /v1/embeddings` - Generate embeddings
- `GET /v1/models` - List available models
- `GET /health` - Health check

**Protocol:**
```json
// Request
{
  "model": "gpt-4o-mini-2024-07-18",
  "prompt": "Hello, world!",
  "temperature": 0.7,
  "max_tokens": 100,
  "stream": false
}

// Response (non-streaming)
{
  "id": "req-123",
  "choices": [{
    "text": "Hello! How can I help you today?",
    "finish_reason": "stop"
  }],
  "usage": {
    "prompt_tokens": 3,
    "completion_tokens": 8,
    "total_tokens": 11
  }
}

// Response (streaming) - Server-Sent Events
data: {"choices": [{"text": "Hello", "finish_reason": null}]}
data: {"choices": [{"text": "!", "finish_reason": null}]}
data: {"choices": [{"text": "", "finish_reason": "stop"}]}
data: [DONE]
```

### TypeScript Client

Located in `src/llm/foundation-models.ts`:

**Key Features:**
- HTTP client with fetch
- Streaming via Server-Sent Events
- Error handling
- Retry logic
- Model selection
- Temperature/max_tokens configuration

**API:**
```typescript
interface FoundationModelClient {
  complete(prompt: string, options?: CompletionOptions): Promise<string>
  stream(prompt: string, options?: CompletionOptions): AsyncIterator<string>
  embed(text: string): Promise<number[]>
  models(): Promise<string[]>
}
```

---

## Rust Crate Architecture

### Crate Structure

```
fm-bridge/
├── Cargo.toml
├── README.md
├── src/
│   ├── lib.rs           # Public API
│   ├── client.rs        # HTTP client
│   ├── types.rs         # Request/response types
│   ├── streaming.rs     # SSE streaming implementation
│   ├── error.rs         # Error types
│   └── retry.rs         # Retry logic
├── tests/
│   ├── integration.rs   # Integration tests (requires Swift bridge running)
│   ├── unit.rs          # Unit tests
│   └── fixtures/        # Test fixtures
└── examples/
    ├── basic.rs         # Simple completion
    ├── streaming.rs     # Streaming example
    └── tools.rs         # Tool calling example
```

### Dependencies

```toml
[dependencies]
# HTTP client
reqwest = { version = "0.12", features = ["json", "stream"] }

# Async runtime
tokio = { version = "1", features = ["full"] }
tokio-stream = "0.1"

# Serialization
serde = { version = "1", features = ["derive"] }
serde_json = "1"

# Error handling
thiserror = "2"
anyhow = "1"

# SSE parsing
eventsource-stream = "0.2"

# Tracing
tracing = "0.1"

[dev-dependencies]
# Testing
tokio-test = "0.4"
wiremock = "0.6"
assert_matches = "1.5"

# Test utilities
tempfile = "3"
```

---

## API Surface

### Core Types

```rust
/// Foundation Model client
pub struct FMClient {
    base_url: String,
    http_client: reqwest::Client,
    default_model: String,
    retry_policy: RetryPolicy,
}

/// Completion request options
#[derive(Debug, Clone, serde::Serialize)]
pub struct CompletionOptions {
    pub model: Option<String>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
    pub top_p: Option<f32>,
    pub stop: Option<Vec<String>>,
    pub stream: bool,
}

/// Completion response
#[derive(Debug, Clone, serde::Deserialize)]
pub struct CompletionResponse {
    pub id: String,
    pub choices: Vec<Choice>,
    pub usage: Usage,
}

/// Choice in completion
#[derive(Debug, Clone, serde::Deserialize)]
pub struct Choice {
    pub text: String,
    pub finish_reason: Option<FinishReason>,
}

/// Finish reason
#[derive(Debug, Clone, Copy, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FinishReason {
    Stop,
    Length,
    ToolCalls,
}

/// Token usage
#[derive(Debug, Clone, serde::Deserialize)]
pub struct Usage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

/// Streaming chunk
#[derive(Debug, Clone)]
pub struct StreamChunk {
    pub text: String,
    pub finish_reason: Option<FinishReason>,
}

/// Embedding response
#[derive(Debug, Clone, serde::Deserialize)]
pub struct EmbeddingResponse {
    pub embedding: Vec<f32>,
    pub usage: Usage,
}

/// Model info
#[derive(Debug, Clone, serde::Deserialize)]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
    pub max_tokens: u32,
}

/// Error type
#[derive(Debug, thiserror::Error)]
pub enum FMError {
    #[error("HTTP request failed: {0}")]
    HttpError(#[from] reqwest::Error),

    #[error("JSON parse error: {0}")]
    JsonError(#[from] serde_json::Error),

    #[error("API error: {status} - {message}")]
    ApiError { status: u16, message: String },

    #[error("Stream error: {0}")]
    StreamError(String),

    #[error("Model not found: {0}")]
    ModelNotFound(String),

    #[error("Rate limit exceeded")]
    RateLimitExceeded,

    #[error("Timeout")]
    Timeout,
}

/// Retry policy
#[derive(Debug, Clone)]
pub struct RetryPolicy {
    pub max_retries: u32,
    pub initial_delay_ms: u64,
    pub max_delay_ms: u64,
    pub exponential_base: f64,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_retries: 3,
            initial_delay_ms: 100,
            max_delay_ms: 5000,
            exponential_base: 2.0,
        }
    }
}
```

### Public API

```rust
impl FMClient {
    /// Create a new client
    pub fn new(base_url: impl Into<String>) -> Self;

    /// Create a new client with custom configuration
    pub fn builder() -> FMClientBuilder;

    /// Complete a prompt (non-streaming)
    pub async fn complete(
        &self,
        prompt: impl Into<String>,
        options: Option<CompletionOptions>,
    ) -> Result<CompletionResponse, FMError>;

    /// Stream a completion
    pub async fn stream(
        &self,
        prompt: impl Into<String>,
        options: Option<CompletionOptions>,
    ) -> Result<impl Stream<Item = Result<StreamChunk, FMError>>, FMError>;

    /// Generate embeddings
    pub async fn embed(
        &self,
        text: impl Into<String>,
    ) -> Result<EmbeddingResponse, FMError>;

    /// List available models
    pub async fn models(&self) -> Result<Vec<ModelInfo>, FMError>;

    /// Health check
    pub async fn health(&self) -> Result<bool, FMError>;
}

/// Builder for FMClient
pub struct FMClientBuilder {
    base_url: String,
    default_model: String,
    retry_policy: RetryPolicy,
    timeout: Duration,
}

impl FMClientBuilder {
    pub fn base_url(mut self, url: impl Into<String>) -> Self;
    pub fn default_model(mut self, model: impl Into<String>) -> Self;
    pub fn retry_policy(mut self, policy: RetryPolicy) -> Self;
    pub fn timeout(mut self, timeout: Duration) -> Self;
    pub fn build(self) -> FMClient;
}
```

---

## Implementation Plan

### Phase 1: Core Types (Week 1, Day 1-2)

**Tasks:**
1. Set up Cargo project structure
2. Define all core types in `types.rs`
3. Implement `serde` serialization/deserialization
4. Add comprehensive doc comments
5. Write unit tests for type conversions

**Deliverable:**
- `fm-bridge` crate compiles
- All types defined
- Unit tests pass

### Phase 2: HTTP Client (Week 1, Day 3-4)

**Tasks:**
1. Implement `FMClient` struct
2. Add `reqwest` HTTP client
3. Implement `complete()` method
4. Implement `embed()` method
5. Implement `models()` method
6. Implement `health()` method
7. Add timeout handling
8. Add error mapping

**Deliverable:**
- Non-streaming API works
- Error handling complete
- Unit tests with `wiremock`

### Phase 3: Streaming (Week 1, Day 5-6)

**Tasks:**
1. Implement SSE parsing in `streaming.rs`
2. Add `stream()` method returning `Stream`
3. Handle `[DONE]` sentinel
4. Add backpressure handling
5. Add error recovery
6. Write streaming tests

**Deliverable:**
- Streaming API works
- Proper Stream implementation
- Integration tests pass

### Phase 4: Retry Logic (Week 1, Day 7)

**Tasks:**
1. Implement `RetryPolicy` in `retry.rs`
2. Add exponential backoff
3. Add max retry limit
4. Add retry for specific errors (network, 5xx)
5. Add tracing for retry attempts

**Deliverable:**
- Retry logic complete
- Configurable policy
- Tests for retry behavior

### Phase 5: Builder Pattern (Week 2, Day 1)

**Tasks:**
1. Implement `FMClientBuilder`
2. Add configuration options
3. Add validation
4. Add convenience constructors

**Deliverable:**
- Ergonomic builder API
- Default configuration
- Examples

### Phase 6: Testing (Week 2, Day 2-3)

**Tasks:**
1. Write unit tests for all modules
2. Write integration tests (require Swift bridge)
3. Write property-based tests
4. Add test fixtures
5. Add benchmarks
6. Measure code coverage

**Deliverable:**
- >90% code coverage
- All tests pass
- Performance benchmarks

### Phase 7: Documentation & Examples (Week 2, Day 4)

**Tasks:**
1. Write comprehensive README
2. Add API documentation
3. Write usage examples
4. Add architecture diagram
5. Document error handling
6. Add migration guide from TypeScript

**Deliverable:**
- Complete documentation
- 3+ examples
- Migration guide

### Phase 8: Integration (Week 2, Day 5)

**Tasks:**
1. Integrate with `hillclimber` (if ready)
2. Integrate with `mechacoder` (if ready)
3. Replace TypeScript client in tests
4. Validate performance
5. Final polish

**Deliverable:**
- Working integration
- Performance validated
- Ready for production use

---

## Testing Strategy

### Unit Tests

**Coverage:**
- Type serialization/deserialization
- Error handling
- Retry logic
- URL construction
- Option building

**Tools:**
- `wiremock` for mocking HTTP
- `assert_matches` for pattern matching
- `tokio-test` for async tests

**Example:**
```rust
#[tokio::test]
async fn test_complete_success() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/v1/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "req-123",
            "choices": [{"text": "Hello!", "finish_reason": "stop"}],
            "usage": {"prompt_tokens": 1, "completion_tokens": 2, "total_tokens": 3}
        })))
        .mount(&mock_server)
        .await;

    let client = FMClient::new(mock_server.uri());
    let response = client.complete("test", None).await.unwrap();

    assert_eq!(response.choices[0].text, "Hello!");
}
```

### Integration Tests

**Prerequisites:**
- Swift bridge running on `localhost:3030`
- Apple device with Foundation Models

**Tests:**
1. Basic completion
2. Streaming completion
3. Embeddings
4. Model listing
5. Error scenarios
6. Concurrent requests
7. Large prompts
8. Long-running streams

**Example:**
```rust
#[tokio::test]
#[ignore] // Requires Swift bridge
async fn test_integration_complete() {
    let client = FMClient::new("http://localhost:3030");

    let response = client
        .complete("Say hello in one word", None)
        .await
        .expect("Failed to complete");

    assert!(!response.choices.is_empty());
    assert!(!response.choices[0].text.is_empty());
}
```

### Property-Based Tests

**Properties:**
- Serialization round-trips
- Retry eventually succeeds (with mock)
- Stream chunks combine to full text
- Options validation

**Example:**
```rust
#[test]
fn prop_completion_options_roundtrip() {
    use quickcheck::quickcheck;

    fn roundtrip(opts: CompletionOptions) -> bool {
        let json = serde_json::to_string(&opts).unwrap();
        let parsed: CompletionOptions = serde_json::from_str(&json).unwrap();
        opts == parsed
    }

    quickcheck(roundtrip as fn(CompletionOptions) -> bool);
}
```

### Performance Tests

**Benchmarks:**
- Completion latency (p50, p99)
- Streaming throughput (tokens/sec)
- Memory usage
- Connection pooling efficiency

**Tools:**
- `criterion` for benchmarks
- `tokio-console` for runtime analysis

**Example:**
```rust
fn bench_complete(c: &mut Criterion) {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let client = FMClient::new("http://localhost:3030");

    c.bench_function("complete_small_prompt", |b| {
        b.to_async(&runtime).iter(|| async {
            client.complete("Hello", None).await.unwrap()
        })
    });
}
```

---

## Performance Targets

### Latency

| Operation | Target | Notes |
|-----------|--------|-------|
| **Complete (small)** | <100ms | 10 token prompt, 20 token completion |
| **Complete (medium)** | <500ms | 100 token prompt, 100 token completion |
| **Complete (large)** | <2s | 1000 token prompt, 500 token completion |
| **Stream first token** | <50ms | Time to first token |
| **Embed** | <50ms | 100 token input |
| **Models** | <10ms | Cached |

### Throughput

| Metric | Target | Notes |
|--------|--------|-------|
| **Concurrent requests** | 10+ | Without blocking |
| **Streaming tokens/sec** | 50+ | On M3 Max |
| **Requests/sec** | 20+ | Small prompts |

### Resource Usage

| Resource | Target | Notes |
|----------|--------|-------|
| **Memory** | <10MB | Per client instance |
| **Connections** | Pooled | Reuse HTTP connections |
| **CPU** | <5% idle | Background overhead |

---

## API Examples

### Example 1: Basic Completion

```rust
use fm_bridge::{FMClient, CompletionOptions};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = FMClient::new("http://localhost:3030");

    let response = client
        .complete("What is the capital of France?", None)
        .await?;

    println!("Response: {}", response.choices[0].text);
    println!("Tokens: {}", response.usage.total_tokens);

    Ok(())
}
```

### Example 2: Streaming

```rust
use fm_bridge::{FMClient, CompletionOptions};
use tokio_stream::StreamExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = FMClient::new("http://localhost:3030");

    let mut stream = client
        .stream("Write a haiku about Rust", None)
        .await?;

    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        print!("{}", chunk.text);
        if chunk.finish_reason.is_some() {
            println!("\nFinished: {:?}", chunk.finish_reason);
        }
    }

    Ok(())
}
```

### Example 3: Custom Configuration

```rust
use fm_bridge::{FMClient, CompletionOptions, RetryPolicy};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = FMClient::builder()
        .base_url("http://localhost:3030")
        .default_model("gpt-4o-mini")
        .retry_policy(RetryPolicy {
            max_retries: 5,
            initial_delay_ms: 200,
            max_delay_ms: 10000,
            exponential_base: 2.0,
        })
        .timeout(Duration::from_secs(30))
        .build();

    let options = CompletionOptions {
        temperature: Some(0.8),
        max_tokens: Some(200),
        ..Default::default()
    };

    let response = client
        .complete("Tell me a joke", Some(options))
        .await?;

    println!("{}", response.choices[0].text);

    Ok(())
}
```

### Example 4: Error Handling

```rust
use fm_bridge::{FMClient, FMError};

#[tokio::main]
async fn main() {
    let client = FMClient::new("http://localhost:3030");

    match client.complete("Hello", None).await {
        Ok(response) => {
            println!("Success: {}", response.choices[0].text);
        }
        Err(FMError::HttpError(e)) => {
            eprintln!("Network error: {}", e);
        }
        Err(FMError::ApiError { status, message }) => {
            eprintln!("API error {}: {}", status, message);
        }
        Err(FMError::Timeout) => {
            eprintln!("Request timed out");
        }
        Err(e) => {
            eprintln!("Other error: {}", e);
        }
    }
}
```

---

## Migration from TypeScript

### Before (TypeScript)

```typescript
import { FoundationModelClient } from "./llm/foundation-models"

const client = new FoundationModelClient("http://localhost:3030")

const response = await client.complete("Hello", {
  temperature: 0.7,
  maxTokens: 100,
})

console.log(response)
```

### After (Rust)

```rust
use fm_bridge::{FMClient, CompletionOptions};

let client = FMClient::new("http://localhost:3030");

let options = CompletionOptions {
    temperature: Some(0.7),
    max_tokens: Some(100),
    ..Default::default()
};

let response = client.complete("Hello", Some(options)).await?;

println!("{}", response.choices[0].text);
```

**Key Differences:**
- `async/await` in Rust requires `tokio` runtime
- Options use `Option<T>` with `..Default::default()`
- Error handling with `Result<T, E>`
- Streaming uses `Stream` trait

---

## Success Criteria

- [ ] All API methods implemented
- [ ] >90% test coverage
- [ ] All integration tests pass
- [ ] Documentation complete
- [ ] Examples working
- [ ] Performance targets met
- [ ] Published to internal registry
- [ ] Used in at least one production component

---

## Timeline

**Total: 2 weeks (10 working days)**

| Phase | Duration | Deliverable |
|-------|----------|-------------|
| Core Types | 2 days | Types defined, unit tests |
| HTTP Client | 2 days | Non-streaming API works |
| Streaming | 2 days | Streaming API works |
| Retry Logic | 1 day | Retry implemented |
| Builder | 1 day | Ergonomic API |
| Testing | 2 days | >90% coverage |
| Documentation | 1 day | Complete docs |
| Integration | 1 day | Production ready |

**Accelerated:** Can be done in 1 week with focused effort

---

## Next Steps

1. **Create crate skeleton** - `cargo new --lib fm-bridge`
2. **Set up CI/CD** - GitHub Actions for tests
3. **Implement core types** - Start with `types.rs`
4. **Add HTTP client** - Implement `client.rs`
5. **Write tests** - TDD approach
6. **Document API** - As you build

---

**Last Updated:** 2025-12-09
**Status:** Ready to Implement
**Owner:** TBD
**Dependencies:** Swift bridge (already exists)
