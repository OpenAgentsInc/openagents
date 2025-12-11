# Telemetry

Structured logging and distributed tracing for OpenAgents binaries.

## Privacy: All Local, No Phone Home

**Your logs stay on your machine.** The telemetry crate provides local logging only - it writes to stdout/stderr on your terminal. There is no automatic upload, no analytics collection, no tracking.

The optional OpenTelemetry support (disabled by default) lets *you* export traces to *your own* observability stack if you choose to set it up. We don't run any collectors.

## Quick Start

Add to your binary:

```rust
fn main() {
    telemetry::init_default("my_binary");
    tracing::info!("Starting up");
}
```

That's it. Logs go to your terminal.

## Configuration

All configuration is via environment variables:

| Variable | Default | Description |
|----------|---------|-------------|
| `RUST_LOG` | `info` | Log filter (e.g., `debug`, `my_crate=trace`) |
| `LOG_FORMAT` | `pretty` | Output format: `json` or `pretty` |
| `TEST_LOG` | unset | Set to enable logs in tests |

### Examples

```bash
# Default pretty output
hillclimber --tasks regex-log

# JSON logs for piping to jq
LOG_FORMAT=json hillclimber --tasks regex-log | jq .

# Debug level
RUST_LOG=debug testgen tb2 run regex-log

# Specific crate debug
RUST_LOG=hillclimber=debug,info testgen tb2 run regex-log
```

## Output Formats

### Pretty (default)

Human-readable, colorized output for development:

```
2024-12-10T21:54:00.000Z  INFO hillclimber Starting task task=regex-log
2024-12-10T21:54:01.234Z DEBUG hillclimber::runner Executing turn turn=1
```

### JSON (bunyan-style)

Machine-readable for log aggregation:

```json
{"v":0,"name":"hillclimber","msg":"Starting task","level":30,"time":"2024-12-10T21:54:00.000Z","task":"regex-log"}
```

Use with `jq` for filtering:

```bash
LOG_FORMAT=json hillclimber --tasks regex-log 2>&1 | jq 'select(.level >= 40)'
```

## Adding Telemetry to a New Binary

### 1. Add dependency

In your crate's `Cargo.toml`:

```toml
[dependencies]
tracing = { workspace = true }
telemetry = { workspace = true }
```

### 2. Initialize in main

```rust
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Simple init (reads RUST_LOG, LOG_FORMAT from env)
    telemetry::init_default("my_binary");

    // Or with verbose flag
    let filter = if args.verbose { "debug" } else { "info" };
    telemetry::init_with_filter("my_binary", filter);

    // Your code here
    tracing::info!("Starting");
    Ok(())
}
```

### 3. Use tracing macros

```rust
use tracing::{debug, info, warn, error, instrument};

#[instrument(skip(client))]
async fn process_task(task_id: &str, client: &Client) -> Result<()> {
    info!(task_id, "Processing task");

    if let Err(e) = client.run().await {
        error!(?e, "Task failed");
        return Err(e);
    }

    debug!("Task complete");
    Ok(())
}
```

## Advanced Features

### Correlation IDs

Track requests across async operations:

```rust
use telemetry::{CorrelationId, correlation_span};

let run_id = CorrelationId::generate();
let _span = correlation_span!("optimization_run", &run_id).entered();

// All logs within this span include correlation_id
tracing::info!(task = "regex-log", "Starting task");
```

Accept external IDs (e.g., from HTTP headers):

```rust
let id = CorrelationId::from_external_or_generate(request.header("X-Request-ID"));
```

### Secret Protection

Prevent API keys from appearing in logs:

```rust
use telemetry::SecretString;

let api_key = SecretString::new(std::env::var("API_KEY")?);
tracing::info!(?api_key);  // Logs: api_key="[REDACTED]"

// Access when actually needed
client.authenticate(api_key.expose_secret());
```

### OpenTelemetry Export (Optional)

For exporting traces to Jaeger, Honeycomb, etc. Enable the feature:

```toml
telemetry = { workspace = true, features = ["otel"] }
```

Configure via environment:

```bash
OTEL_EXPORTER_OTLP_ENDPOINT=http://localhost:4317 \
OTEL_SERVICE_NAME=hillclimber \
hillclimber --tasks regex-log
```

**Note:** This sends traces to YOUR configured endpoint. We don't run any collectors.

## Testing

Logs are suppressed in tests by default. Enable them:

```bash
TEST_LOG=1 cargo test
```

In test code:

```rust
#[test]
fn my_test() {
    telemetry::init_test();  // Only outputs if TEST_LOG is set
    // ...
}
```

## Current Binaries Using Telemetry

| Binary | Crate | Init |
|--------|-------|------|
| `hillclimber` | `crates/hillclimber` | `init_with_filter` (respects `--verbose`) |
| `testgen` | `crates/testgen` | `init_default` |
| `tbench` | `crates/harbor` | `init_with_filter` (respects `--verbose`) |

## Architecture

```
telemetry/
├── lib.rs          # init_default(), init_with_filter(), init_with_config()
├── config.rs       # TelemetryConfig, LogFormat, OtelConfig
├── correlation.rs  # CorrelationId, correlation_span! macro
├── secrets.rs      # SecretString wrapper
├── error.rs        # TelemetryError
└── layers/
    ├── json.rs     # Bunyan-style JSON output
    ├── pretty.rs   # Terminal-friendly output
    └── otel.rs     # OpenTelemetry OTLP export (feature-gated)
```

## FAQ

**Q: Does this send data anywhere?**

A: No. Logs go to stdout/stderr only. The optional OpenTelemetry feature lets you export to your own infrastructure if you configure it.

**Q: Why not just use `env_logger`?**

A: `tracing` provides structured logging (key-value pairs), spans for async context, and better performance. The telemetry crate wraps this with sensible defaults.

**Q: How do I see debug logs for just my crate?**

A: `RUST_LOG=my_crate=debug,info` - sets `my_crate` to debug, everything else to info.

**Q: How do I pipe JSON logs to a file?**

A: `LOG_FORMAT=json hillclimber --tasks regex-log 2>&1 | tee logs.jsonl`
