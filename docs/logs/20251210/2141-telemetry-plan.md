# Telemetry Crate Implementation Plan

Create `crates/telemetry/` with structured logging, distributed tracing, and correlation ID support inspired by Zero to Production Chapter 4.

## Requirements Confirmed
- **Log Format**: JSON (bunyan-style) + pretty terminal, switchable via `LOG_FORMAT` env var
- **OpenTelemetry**: OTLP export support (feature-gated)
- **Correlation IDs**: Generate UUIDs or accept external IDs

---

## Crate Structure

```
crates/telemetry/
├── Cargo.toml
└── src/
    ├── lib.rs              # Public API: get_subscriber(), init_subscriber()
    ├── config.rs           # TelemetryConfig, LogFormat, env var parsing
    ├── error.rs            # TelemetryError (thiserror)
    ├── correlation.rs      # CorrelationId type + correlation_span! macro
    ├── secrets.rs          # SecretString wrapper (redacts in logs)
    ├── layers/
    │   ├── mod.rs          # Layer composition
    │   ├── json.rs         # Bunyan-style JSON formatting
    │   ├── pretty.rs       # Terminal-friendly pretty output
    │   └── otel.rs         # OpenTelemetry OTLP export (feature-gated)
    └── tests/
        ├── mod.rs
        ├── fixtures/
        │   ├── mod.rs
        │   └── test_subscriber.rs
        ├── config_tests.rs
        ├── correlation_tests.rs
        ├── secrets_tests.rs
        └── format_tests.rs
```

---

## Public API

### Core Functions (zero2prod style)

```rust
// Simple initialization
pub fn init_default(name: &str);

// Full control
pub fn get_subscriber(name: String, default_filter: String) -> impl Subscriber + Send + Sync;
pub fn init_subscriber(subscriber: impl Subscriber + Send + Sync);

// Programmatic config
pub fn get_subscriber_with_config(name: String, default_filter: String, config: TelemetryConfig) -> impl Subscriber + Send + Sync;
```

### Key Types

```rust
pub enum LogFormat { Json, Pretty }
pub struct TelemetryConfig { filter, format, otel, test_mode }
pub struct CorrelationId(String);  // Generate or accept external
pub struct SecretString { inner: String }  // Debug/Display shows [REDACTED]
```

### Macros

```rust
// Create span with correlation ID
correlation_span!("operation_name", correlation_id)
correlation_span!("operation_name", correlation_id, field = %value)
```

---

## Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `RUST_LOG` | `info` | Log filter directive |
| `LOG_FORMAT` | `pretty` | `json` or `pretty` |
| `OTEL_EXPORTER_OTLP_ENDPOINT` | - | Enables OTLP export |
| `OTEL_SERVICE_NAME` | app name | Service name for traces |
| `TEST_LOG` | - | If set, enables logs in tests |

---

## Dependencies

### Add to workspace Cargo.toml

```toml
[workspace.dependencies]
tracing-subscriber = { version = "0.3", features = ["env-filter", "json", "fmt"] }
tracing-bunyan-formatter = "0.3"
tracing-log = "0.2"

# Optional OTEL (used when feature enabled)
opentelemetry = "0.21"
opentelemetry-otlp = { version = "0.14", features = ["tonic"] }
opentelemetry_sdk = { version = "0.21", features = ["rt-tokio"] }
tracing-opentelemetry = "0.22"
```

### crates/telemetry/Cargo.toml

```toml
[package]
name = "telemetry"
version.workspace = true
edition.workspace = true
authors.workspace = true
license.workspace = true

[features]
default = []
test-support = []
otel = ["opentelemetry", "opentelemetry-otlp", "opentelemetry_sdk", "tracing-opentelemetry"]

[dependencies]
tracing = { workspace = true }
tracing-subscriber = { workspace = true }
tracing-bunyan-formatter = { workspace = true }
tracing-log = { workspace = true }
uuid = { workspace = true }
thiserror = { workspace = true }
serde = { workspace = true }

# Optional OTEL
opentelemetry = { workspace = true, optional = true }
opentelemetry-otlp = { workspace = true, optional = true }
opentelemetry_sdk = { workspace = true, optional = true }
tracing-opentelemetry = { workspace = true, optional = true }

[dev-dependencies]
tokio = { workspace = true, features = ["full", "test-util", "macros"] }
serde_json = { workspace = true }
regex = { workspace = true }
```

---

## Implementation Steps

### 1. Create crate structure
- Create `crates/telemetry/` directory
- Create `Cargo.toml` with dependencies
- Add `"crates/telemetry"` to workspace members in root `Cargo.toml`
- Add new workspace dependencies

### 2. Implement core modules
- `error.rs` - TelemetryError enum with thiserror
- `config.rs` - TelemetryConfig, LogFormat, builder pattern
- `secrets.rs` - SecretString with redacted Debug/Display
- `correlation.rs` - CorrelationId + validation + macro

### 3. Implement layers
- `layers/mod.rs` - Layer composition logic
- `layers/json.rs` - BunyanFormattingLayer + JsonStorageLayer
- `layers/pretty.rs` - fmt::layer().pretty()
- `layers/otel.rs` - OpenTelemetry layer (feature-gated)

### 4. Implement lib.rs
- Re-exports for public API
- `get_subscriber()` - compose layers based on config
- `init_subscriber()` - set global default
- `init_default()` - convenience function
- Test mode handling (sink to void unless TEST_LOG set)

### 5. Write tests
- `tests/fixtures/` - TestSubscriber that captures to buffer
- `tests/config_tests.rs` - env var parsing, builder pattern
- `tests/correlation_tests.rs` - generate, validate external, fallback
- `tests/secrets_tests.rs` - Debug/Display redaction
- `tests/format_tests.rs` - JSON validity, field presence

---

## Test Plan

### Config Tests
- [x] Parses RUST_LOG from env
- [x] Parses LOG_FORMAT (json, pretty, unknown defaults to pretty)
- [x] Parses OTEL_* env vars
- [x] Builder pattern works
- [x] Default values correct

### Correlation ID Tests
- [x] `generate()` creates unique UUIDs
- [x] `from_external()` accepts valid IDs
- [x] `from_external()` rejects empty string
- [x] `from_external()` rejects > 128 chars
- [x] `from_external_or_generate()` falls back correctly
- [x] Display/Debug work

### Secret Tests
- [x] Debug shows `[REDACTED]`
- [x] Display shows `[REDACTED]`
- [x] Serialize shows `[REDACTED]`
- [x] `expose_secret()` returns actual value
- [x] Equality works on inner value

### Format Tests
- [x] JSON output is valid JSON
- [x] JSON contains expected fields (timestamp, level, message)
- [x] Pretty output is human-readable
- [x] Test mode suppresses output
- [x] TEST_LOG=true enables output

---

## Usage After Implementation

### CLI Binary (simple)
```rust
use telemetry::init_default;

fn main() {
    init_default("hillclimber");
    tracing::info!("Starting");
}
```

### CLI Binary (with correlation ID)
```rust
use telemetry::{init_default, CorrelationId, correlation_span};

fn main() {
    init_default("hillclimber");
    let id = CorrelationId::generate();
    let _span = correlation_span!("run", id).entered();
    tracing::info!("Starting with correlation");
}
```

### Library Code
```rust
use telemetry::CorrelationId;

#[tracing::instrument(skip(client), fields(correlation_id = %id))]
pub async fn process(id: &CorrelationId, client: &Client) -> Result<()> {
    tracing::info!("Processing");
    Ok(())
}
```

### Secret Protection
```rust
use telemetry::SecretString;

let api_key = SecretString::new(std::env::var("API_KEY")?);
tracing::info!(?api_key);  // Logs: api_key="[REDACTED]"
```

---

## Files to Modify

| File | Change |
|------|--------|
| `/Cargo.toml` | Add `telemetry` to members, add workspace deps |
| `crates/telemetry/Cargo.toml` | **NEW** |
| `crates/telemetry/src/lib.rs` | **NEW** |
| `crates/telemetry/src/error.rs` | **NEW** |
| `crates/telemetry/src/config.rs` | **NEW** |
| `crates/telemetry/src/correlation.rs` | **NEW** |
| `crates/telemetry/src/secrets.rs` | **NEW** |
| `crates/telemetry/src/layers/mod.rs` | **NEW** |
| `crates/telemetry/src/layers/json.rs` | **NEW** |
| `crates/telemetry/src/layers/pretty.rs` | **NEW** |
| `crates/telemetry/src/layers/otel.rs` | **NEW** |
| `crates/telemetry/src/tests/mod.rs` | **NEW** |
| `crates/telemetry/src/tests/fixtures/mod.rs` | **NEW** |
| `crates/telemetry/src/tests/fixtures/test_subscriber.rs` | **NEW** |
| `crates/telemetry/src/tests/config_tests.rs` | **NEW** |
| `crates/telemetry/src/tests/correlation_tests.rs` | **NEW** |
| `crates/telemetry/src/tests/secrets_tests.rs` | **NEW** |
| `crates/telemetry/src/tests/format_tests.rs` | **NEW** |
