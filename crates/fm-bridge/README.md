# FM Bridge

Rust client for Apple Foundation Models via HTTP bridge. Provides type-safe access to Apple's on-device LLM inference with an OpenAI-compatible API.

## Features

### ✅ Supported

- **Chat Completions** - OpenAI-compatible `/v1/chat/completions` endpoint
- **Model Listing** - Get available Apple models
- **Health Checks** - Verify bridge and model availability
- **Guided Generation** - Structured output with pre-defined schemas (via Swift bridge)
- **Token Tracking** - Approximate token counts for usage monitoring
- **CLI Tool** - Easy command-line interface
- **Async/await** - Tokio-based async runtime
- **Type-safe errors** - Comprehensive error handling

### ⚠️  Partial Support

- **Streaming** - Client has streaming code, but Swift bridge doesn't implement SSE yet
- **Custom schemas** - Schema type hints supported, dynamic schemas TODO

### ❌ Not Supported

- **Multi-turn conversations** - Each request is independent (no session context in bridge)
- **Function calling** - No tool use API
- **System messages** - Treated same as user messages by the model
- **Embeddings** - Not exposed by FoundationModels framework

## Quick Start

**1. Build the Swift bridge:**
```bash
cd ../  # Go to openagents root
bun run bridge:build
```

**2. Start the bridge:**
```bash
bun run bridge  # Starts on port 11435
# or with custom port:
bin/foundation-bridge 3030
```

**3. Install the Rust CLI:**
```bash
cd fm-bridge
cargo install --path .
```

**4. Test it:**
```bash
fm --url http://localhost:3030 complete "Hello, world!"
```

## Requirements

**For the Swift Bridge:**
- macOS 15.1+ (Sequoia)
- Apple Silicon (M1/M2/M3/M4)
- Xcode 16+
- Swift 6.0+

**For the Rust Client:**
- Rust 1.70+
- Tokio async runtime

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
fm-bridge = { path = "../fm-bridge" }
tokio = { version = "1", features = ["full"] }
```

## Library Usage

### Basic Completion

```rust
use fm_bridge::{FMClient, CompletionOptions};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = FMClient::new();

    let response = client
        .complete("What is the capital of France?", None)
        .await?;

    println!("Response: {}", response.choices[0].message.content);
    Ok(())
}
```

### Streaming (Not Yet Implemented)

```rust
use fm_bridge::FMClient;
use tokio_stream::StreamExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = FMClient::new();

    // Note: Swift bridge doesn't implement SSE yet, so this will not work
    let mut stream = client.stream("Write a haiku about Rust", None).await?;

    while let Some(chunk) = stream.next().await {
        match chunk {
            Ok(chunk) => {
                print!("{}", chunk.text);
                if chunk.finish_reason.is_some() {
                    break;
                }
            }
            Err(e) => eprintln!("Error: {}", e),
        }
    }

    Ok(())
}
```

### Custom Configuration

```rust
use fm_bridge::{FMClient, CompletionOptions};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = FMClient::builder()
        .base_url("http://localhost:3030")
        .default_model("apple-foundation-model")
        .timeout(Duration::from_secs(60))
        .build();

    let options = CompletionOptions {
        temperature: Some(0.8),
        max_tokens: Some(200),
        ..Default::default()
    };

    let response = client.complete("Tell me a joke", Some(options)).await?;
    println!("{}", response.choices[0].message.content);
    Ok(())
}
```

## CLI Usage

### Build

```bash
cd fm-bridge
cargo build --release
```

### Install

```bash
cargo install --path .
```

### Commands

#### Complete a Prompt

```bash
# Basic completion
fm complete "What is Rust?"

# With options
fm complete "Write a poem" --temperature 0.9 --max-tokens 100

# Streaming (not yet implemented in Swift bridge)
fm complete "Count to 10" --stream
```

#### List Models

```bash
fm models
```

#### Health Check

```bash
fm health
```

#### Custom URL

```bash
fm --url http://localhost:3030 complete "Hello"
```

## Guided Generation

The Swift bridge supports **guided generation** for guaranteed structured output. This uses Apple's `@Generable` framework to constrain model output to specific schemas.

**Note:** The Rust client doesn't directly implement guided generation - you send requests to the Swift bridge with `response_format` in the JSON body.

**Available pre-defined schemas:**
- `test_generation` - Generate test cases from task descriptions
- `environment_aware_test_generation` - Generate tests with environment context
- `tool_call` - Constrained tool selection for agents

**Example using cURL:**
```bash
curl http://localhost:3030/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{
    "model": "apple-foundation-model",
    "messages": [{"role": "user", "content": "Generate 3 tests for validating email addresses"}],
    "response_format": {
      "type": "json_schema",
      "schema_type": "test_generation"
    }
  }'
```

For detailed documentation on guided generation, see:
- [docs/foundation-models/guided-generation.md](../docs/foundation-models/guided-generation.md)
- [docs/foundation-models/README.md](../docs/foundation-models/README.md)

## API Endpoints

The Swift bridge exposes these OpenAI-compatible endpoints:

- `POST /v1/chat/completions` - Chat completions (with optional guided generation)
- `GET /v1/models` - List available models
- `GET /health` - Health check + model availability

**Note:** The endpoints use `/v1/chat/completions` (OpenAI chat API), NOT `/v1/completions` (legacy completions API).

## Development

### Run Tests

```bash
cargo test
```

### Run CLI

```bash
cargo run --bin fm -- complete "Hello, world!"
```

### With Streaming

```bash
cargo run --bin fm -- complete "Write a story" --stream
```

## Troubleshooting

### Connection Refused

**Problem:** `error sending request for url (http://localhost:3030/health)`

**Solution:** Start the Swift bridge first:
```bash
cd ..  # Go to openagents root
bun run bridge
```

### API 404 Error

**Problem:** `Error: ApiError { status: 404, message: "Not found: POST /v1/completions" }`

**Solution:** You're using an old version. The current client uses `/v1/chat/completions`. Rebuild and reinstall:
```bash
cargo build --release
cargo install --path . --force
```

### Model Unavailable

**Problem:** Health check shows `model_available: false`

**Requirements:**
- macOS 15.1+ (Sequoia)
- Apple Silicon (M1/M2/M3/M4)
- Apple Intelligence enabled in System Settings
- On-device model downloaded

### SIGKILL When Starting Bridge

**Problem:** `bin/foundation-bridge` gets killed immediately

**Solution:** Rebuild to create wrapper script:
```bash
cd ..
bun run bridge:build
```

The wrapper preserves the binary's code signature needed for FoundationModels access.

## Documentation

- [Setup & Troubleshooting Guide](../docs/foundation-models/bridge-setup.md) - Detailed setup, architecture, known issues
- [Guided Generation Guide](../docs/foundation-models/guided-generation.md) - Structured output with schemas
- [Main Documentation](../docs/foundation-models/README.md) - Complete Foundation Models integration guide

## License

MIT
