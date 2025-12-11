# Telemetry Crate Implementation

**Date:** 2024-12-10 21:54
**Author:** Claude (Opus 4.5)

## Overview

Implemented a comprehensive `crates/telemetry/` crate for structured logging and distributed tracing, inspired by Zero to Production Chapter 4 (pages 112-155). The crate provides a unified telemetry initialization API with support for JSON and pretty log formats, correlation IDs for request tracking, and optional OpenTelemetry export.

## Motivation

The existing codebase had minimal tracing setup - just basic `tracing_subscriber::fmt::init()` calls scattered across binaries. This made it difficult to:
- Switch between JSON logs (for production/log aggregation) and pretty logs (for development)
- Track requests across service boundaries with correlation IDs
- Protect sensitive data from appearing in logs
- Export traces to observability platforms

## Implementation Details

### Crate Structure

```
crates/telemetry/
├── Cargo.toml
└── src/
    ├── lib.rs              # Public API: init_default(), init_with_filter(), init_with_config()
    ├── config.rs           # TelemetryConfig, LogFormat, OtelConfig
    ├── error.rs            # TelemetryError (thiserror)
    ├── correlation.rs      # CorrelationId + correlation_span! macro
    ├── secrets.rs          # SecretString wrapper (redacts in logs)
    └── layers/
        ├── mod.rs
        ├── json.rs         # Bunyan-style JSON formatting
        ├── pretty.rs       # Terminal-friendly output
        └── otel.rs         # OpenTelemetry OTLP export (feature-gated)
```

### Key Design Decisions

#### 1. Simplified Public API

Instead of the zero2prod pattern of `get_subscriber()` + `init_subscriber()` which returns `impl Subscriber`, I simplified to direct init functions:

```rust
// Simple - reads config from env vars
telemetry::init_default("my_app");

// Custom filter
telemetry::init_with_filter("my_app", "debug");

// Full control
telemetry::init_with_config("my_app", config);
```

This avoids the complex type issues when trying to return different subscriber types from a match expression. The tradeoff is less composability, but the API is cleaner for common use cases.

#### 2. Environment Variable Configuration

| Variable | Default | Description |
|----------|---------|-------------|
| `RUST_LOG` | `info` | Standard tracing filter directive |
| `LOG_FORMAT` | `pretty` | Switch between `json` and `pretty` |
| `OTEL_EXPORTER_OTLP_ENDPOINT` | - | Enables OTLP if set |
| `OTEL_SERVICE_NAME` | app name | Service name in traces |
| `TEST_LOG` | - | Enables logs in test mode |

#### 3. Correlation IDs

The `CorrelationId` type supports both generating new UUIDs and accepting external IDs (e.g., from HTTP headers):

```rust
// Generate new
let id = CorrelationId::generate();

// Accept external (with validation)
let id = CorrelationId::from_external(header_value)?;

// Accept external or fall back to generated
let id = CorrelationId::from_external_or_generate(header_value);
```

The `correlation_span!` macro creates spans with the correlation ID automatically:

```rust
let _span = correlation_span!("process_request", &id).entered();
// All logs within this span include correlation_id field
```

#### 4. Secret Protection

`SecretString` wraps sensitive data and redacts it in Debug, Display, and Serialize:

```rust
let api_key = SecretString::new(env::var("API_KEY")?);
tracing::info!(?api_key);  // Logs: api_key="[REDACTED]"
println!("{:?}", api_key); // Prints: [REDACTED]

// Access actual value when needed
client.auth(api_key.expose_secret());
```

#### 5. Feature-Gated OpenTelemetry

The `otel` feature enables OpenTelemetry OTLP export. This keeps the default dependency footprint small:

```toml
# Basic usage
telemetry = { path = "crates/telemetry" }

# With OTEL support
telemetry = { path = "crates/telemetry", features = ["otel"] }
```

### Dependencies Added to Workspace

```toml
tracing-subscriber = { version = "0.3", features = ["env-filter", "json", "fmt"] }
tracing-bunyan-formatter = "0.3"
tracing-log = "0.2"

# OpenTelemetry (optional)
opentelemetry = "0.21"
opentelemetry-otlp = { version = "0.14", features = ["tonic"] }
opentelemetry_sdk = { version = "0.21", features = ["rt-tokio"] }
tracing-opentelemetry = "0.22"
```

### Test Coverage

All 29 tests pass:
- **20 unit tests** covering config parsing, correlation ID validation, secret redaction
- **9 doc tests** verifying example code compiles and runs

Key test cases:
- Log format parsing (json, pretty, unknown defaults)
- Config builder pattern
- Correlation ID uniqueness
- External correlation ID validation (empty, too long)
- Secret redaction in Debug, Display, Serialize
- Secret equality based on inner value

## Usage Examples

### CLI Binary (Simple)

```rust
fn main() {
    telemetry::init_default("hillclimber");
    tracing::info!("Starting HillClimber");
    // ...
}
```

### CLI Binary (With Correlation)

```rust
fn main() {
    telemetry::init_default("hillclimber");

    let run_id = CorrelationId::generate();
    let _span = correlation_span!("optimization_run", &run_id).entered();

    tracing::info!(task = "regex-log", "Starting task");
    // All nested logs include correlation_id
}
```

### Library Code

```rust
use telemetry::CorrelationId;

#[tracing::instrument(skip(client), fields(correlation_id = %id))]
pub async fn process_task(id: &CorrelationId, client: &Client) -> Result<()> {
    tracing::info!("Processing");
    // ...
}
```

### Production JSON Output

With `LOG_FORMAT=json`:

```json
{"v":0,"name":"hillclimber","msg":"Starting task","level":30,"hostname":"mac","pid":12345,"time":"2024-12-10T21:54:00.000Z","correlation_id":"a1b2c3d4-...","task":"regex-log"}
```

### Development Pretty Output

With `LOG_FORMAT=pretty` (default):

```
2024-12-10T21:54:00.000Z  INFO hillclimber Starting task correlation_id=a1b2c3d4-... task=regex-log
```

## Files Created/Modified

### New Files
- `crates/telemetry/Cargo.toml`
- `crates/telemetry/src/lib.rs`
- `crates/telemetry/src/config.rs`
- `crates/telemetry/src/error.rs`
- `crates/telemetry/src/correlation.rs`
- `crates/telemetry/src/secrets.rs`
- `crates/telemetry/src/layers/mod.rs`
- `crates/telemetry/src/layers/json.rs`
- `crates/telemetry/src/layers/pretty.rs`
- `crates/telemetry/src/layers/otel.rs`

### Modified Files
- `Cargo.toml` (root) - added telemetry to members, added workspace dependencies

## Future Improvements

1. **Integrate with existing crates** - Update hillclimber, gym, etc. to use telemetry crate
2. **Add span events** - Helper macros for common patterns like timing
3. **Metrics layer** - Add prometheus/metrics support alongside tracing
4. **Error context** - Integration with anyhow/eyre for error chain logging

## References

- Zero to Production in Rust, Chapter 4: Telemetry
- [tracing-subscriber docs](https://docs.rs/tracing-subscriber)
- [tracing-bunyan-formatter docs](https://docs.rs/tracing-bunyan-formatter)
- [OpenTelemetry Rust](https://opentelemetry.io/docs/languages/rust/)
