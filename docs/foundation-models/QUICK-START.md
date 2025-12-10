# Foundation Models Quick Start

**For Coding Agents:** This is the fastest path to using Apple Foundation Models for local inference.

## 5-Minute Setup

### 1. Build the Bridge

```bash
bun run bridge:build
```

### 2. Start the Server

```bash
bun run bridge
```

Expected output:
```
Foundation Models HTTP Bridge
==============================
Starting server on port 11435...
Endpoints:
  GET  /health              - Check server and model status
  GET  /v1/models           - List available models
  POST /v1/chat/completions - Chat completion (OpenAI-compatible)

Press Ctrl+C to stop.
Foundation Models: Available
Server listening on port 11435
```

### 3. Test with cURL

```bash
# Health check
curl http://localhost:11435/health

# Chat completion
curl http://localhost:11435/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{
    "model": "apple-foundation-model",
    "messages": [{"role": "user", "content": "Say hello in one word"}]
  }'
```

## Using the Rust CLI

### Install

```bash
cd fm-bridge
cargo install --path .
```

### Basic Usage

```bash
# Health check
fm health

# List models
fm models

# Complete a prompt
fm complete "What is Rust?"

# With custom server
fm --url http://localhost:11435 complete "Hello"
```

## API Reference

### POST /v1/chat/completions

**Request:**
```json
{
  "model": "apple-foundation-model",
  "messages": [
    {"role": "user", "content": "Your prompt here"}
  ],
  "temperature": 0.7,
  "max_tokens": 1000
}
```

**Response:**
```json
{
  "id": "fm-...",
  "object": "chat.completion",
  "created": 1765331418,
  "model": "apple-foundation-model",
  "choices": [{
    "index": 0,
    "message": {
      "role": "assistant",
      "content": "Generated response"
    },
    "finish_reason": "stop"
  }],
  "usage": {
    "prompt_tokens": 10,
    "completion_tokens": 20,
    "total_tokens": 30
  }
}
```

### Guided Generation (Structured Output)

**Request with schema:**
```json
{
  "model": "apple-foundation-model",
  "messages": [{"role": "user", "content": "Generate 3 tests for email validation"}],
  "response_format": {
    "type": "json_schema",
    "schema_type": "test_generation"
  }
}
```

**Available schemas:**
- `test_generation` - Test cases with input/output/reasoning
- `environment_aware_test_generation` - Tests with environment context
- `tool_call` - Constrained tool selection

## Rust Client Library

```rust
use fm_bridge::{FMClient, CompletionOptions};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = FMClient::builder()
        .base_url("http://localhost:11435")
        .build();

    let response = client
        .complete("What is 2+2?", None)
        .await?;

    println!("{}", response.choices[0].message.content);
    Ok(())
}
```

## TypeScript/Bun Client

```typescript
import { createFMClient } from "./src/llm/foundation-models.js";
import { Effect } from "effect";

const client = createFMClient({ port: 11435 });

const response = await Effect.runPromise(
  client.chat({
    messages: [{ role: "user", content: "Hello!" }]
  })
);

console.log(response.choices[0].message.content);
```

## Common Issues

### "Connection refused"
**Fix:** Start the bridge with `bun run bridge`

### "Model not available"
**Requires:**
- macOS 15.1+ (Sequoia)
- Apple Silicon (M1/M2/M3/M4)
- Apple Intelligence enabled in System Settings

### "SIGKILL when running bridge"
**Fix:** Rebuild with `bun run bridge:build` (creates wrapper script)

### "404 Not Found: POST /v1/completions"
**Fix:** Update Rust client - uses `/v1/chat/completions` now

## What's Supported

| Feature | Status |
|---------|--------|
| Chat completions | ✅ Working |
| Model listing | ✅ Working |
| Health checks | ✅ Working |
| Guided generation | ✅ Working (via Swift bridge) |
| Token tracking | ✅ Working (estimated) |
| Streaming | ⚠️  Not yet (client has code, bridge doesn't) |
| Multi-turn | ❌ Not supported (stateless) |
| Function calling | ❌ Not supported |
| Embeddings | ❌ Not supported |

## Performance

- **First token:** ~100-500ms (model loading)
- **Throughput:** ~50-100 tokens/sec
- **Memory:** ~2-4GB for model + bridge

## Documentation

- **Full Setup Guide:** [bridge-setup.md](./bridge-setup.md)
- **Guided Generation:** [guided-generation.md](./guided-generation.md)
- **Complete Reference:** [README.md](./README.md)
- **Rust Client:** [fm-bridge/README.md](../../fm-bridge/README.md)

## Default Ports

- **Bridge Default:** 11435 (one higher than Ollama's 11434)
- **Custom Port:** `bin/foundation-bridge 3030` or `createFMClient({ port: 3030 })`

## Next Steps

1. Read [guided-generation.md](./guided-generation.md) for structured output
2. Check [bridge-setup.md](./bridge-setup.md) for troubleshooting
3. See TypeScript client docs in [README.md](./README.md)
