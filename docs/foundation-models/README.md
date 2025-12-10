# Foundation Models Integration

Apple Foundation Models integration for local LLM inference on macOS 26+ with Apple Intelligence.

## Quick Links

- **[Quick Start Guide](./QUICK-START.md)** - 5-minute setup for coding agents
- **[Guided Generation](./guided-generation.md)** - Constrained output for guaranteed valid JSON (4x faster, 100% reliable)
- **[Framework Capabilities](./framework-capabilities.md)** - Complete API surface of Apple's FoundationModels framework
- **[Setup & Troubleshooting](./bridge-setup.md)** - Detailed setup, known issues, fixes

## Overview

Foundation Models is Apple's on-device language model framework, part of Apple Intelligence. This integration provides:

- **Swift HTTP Bridge**: A lightweight server exposing an OpenAI-compatible API
- **Bun/TypeScript Client**: Effect-based client with auto-start capability
- **TerminalBench Integration**: Use FM as a model provider for benchmarking

The bridge runs locally and provides the same API shape as OpenAI/Ollama, making it a drop-in replacement for local inference.

## Requirements

| Requirement | Details |
|-------------|---------|
| **macOS** | 26.0 or later (Tahoe) |
| **Hardware** | Apple Silicon (M1/M2/M3/M4) |
| **Apple Intelligence** | Must be enabled in System Settings |
| **Xcode** | 26.0+ with Swift 6.2 toolchain |

### Checking Availability

The server reports availability via the `/health` endpoint:

```bash
curl http://localhost:11435/health
```

```json
{
  "status": "ok",
  "model_available": true,
  "version": "1.0.0",
  "platform": "macOS"
}
```

If `model_available` is `false`, check:
1. Apple Intelligence is enabled in System Settings → Apple Intelligence & Siri
2. The on-device model has finished downloading
3. You're running on Apple Silicon hardware

## Architecture

```
┌─────────────────┐     HTTP      ┌──────────────────┐     Native     ┌─────────────────┐
│   Bun/Node.js   │ ──────────── │  foundation-     │ ─────────────  │  Apple          │
│   Application   │   :11435     │  bridge (Swift)  │   Framework    │  Foundation     │
│                 │              │                  │                │  Models         │
└─────────────────┘              └──────────────────┘                └─────────────────┘
```

- **Port 11435**: Default port (one higher than Ollama's 11434)
- **OpenAI-compatible**: Same API shape as `/v1/chat/completions`
- **On-demand**: Server auto-starts when first request is made

## Building the Bridge

### Quick Build

```bash
cd swift/foundation-bridge
./build.sh
```

The binary is placed at `bin/foundation-bridge`.

### Manual Build

```bash
cd swift/foundation-bridge
swift build -c release
cp .build/release/foundation-bridge ../../bin/
```

### Build Requirements

The `Package.swift` requires:
- `swift-tools-version:6.2` (for macOS 26 platform support)
- `-parse-as-library` flag (for async main)

## Running the Server

### Manual Start

```bash
# Default port (11435)
./bin/foundation-bridge

# Custom port
./bin/foundation-bridge 8080
```

Output:
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

### Auto-Start from Bun

The Bun client automatically starts the server if not running:

```typescript
import { createFMClient } from "./src/llm/foundation-models.js";
import { Effect } from "effect";

const client = createFMClient(); // autoStart: true by default

// Server starts automatically on first request
const response = await Effect.runPromise(
  client.chat({
    messages: [{ role: "user", content: "Hello!" }]
  })
);
```

## API Reference

### GET /health

Check server status and model availability.

**Response:**
```json
{
  "status": "ok",
  "model_available": true,
  "version": "1.0.0",
  "platform": "macOS"
}
```

| Field | Type | Description |
|-------|------|-------------|
| `status` | string | `"ok"` or `"degraded"` |
| `model_available` | boolean | Whether Foundation Models can be used |
| `version` | string | Bridge version |
| `platform` | string | Always `"macOS"` |

### GET /v1/models

List available models (OpenAI-compatible).

**Response:**
```json
{
  "object": "list",
  "data": [
    {
      "id": "apple-foundation-model",
      "object": "model",
      "created": 1764989354,
      "owned_by": "apple"
    }
  ]
}
```

### POST /v1/chat/completions

Generate a chat completion.

**Request:**
```json
{
  "model": "apple-foundation-model",
  "messages": [
    {"role": "system", "content": "You are a helpful assistant."},
    {"role": "user", "content": "What is 2+2?"}
  ],
  "temperature": 0.7,
  "max_tokens": 1000,
  "stream": false
}
```

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `model` | string | `"apple-foundation-model"` | Model identifier (ignored, only one model) |
| `messages` | array | required | Array of chat messages |
| `temperature` | number | - | Sampling temperature (currently ignored by FM) |
| `max_tokens` | number | - | Max tokens to generate (currently ignored by FM) |
| `stream` | boolean | `false` | Streaming not yet supported |

**Message Format:**
```json
{
  "role": "system" | "user" | "assistant" | "tool",
  "content": "string"
}
```

**Response:**
```json
{
  "id": "fm-f3abadec-9b19-4aa5-9a4d-ba51ab1bc17f",
  "object": "chat.completion",
  "created": 1764989354,
  "model": "apple-foundation-model",
  "choices": [
    {
      "index": 0,
      "message": {
        "role": "assistant",
        "content": "2+2 equals 4."
      },
      "finish_reason": "stop"
    }
  ],
  "usage": {
    "prompt_tokens": 27,
    "completion_tokens": 7,
    "total_tokens": 34
  }
}
```

| Field | Description |
|-------|-------------|
| `id` | Unique completion ID (prefixed with `fm-`) |
| `created` | Unix timestamp |
| `choices[].message.content` | Generated text |
| `choices[].finish_reason` | Always `"stop"` (no streaming) |
| `usage` | Token counts (estimated: ~4 chars/token) |

### Error Responses

All errors follow OpenAI's error format:

```json
{
  "error": {
    "message": "Description of the error",
    "type": "error_type",
    "code": "error_code"
  }
}
```

| HTTP Status | Code | Description |
|-------------|------|-------------|
| 400 | `invalid_request` | Malformed JSON or missing fields |
| 503 | `model_unavailable` | Foundation Models not available on device |
| 500 | `server_error` | Internal server error |

## TypeScript/Bun Client

### Installation

The client is built into OpenAgents at `src/llm/foundation-models.ts`.

### Basic Usage

```typescript
import { createFMClient, checkFMHealth, isMacOS } from "./src/llm/foundation-models.js";
import { Effect } from "effect";

// Check if we're on macOS
if (!isMacOS()) {
  console.log("Foundation Models requires macOS");
  process.exit(1);
}

// Create client
const client = createFMClient();

// Make a request
const response = await Effect.runPromise(
  client.chat({
    messages: [
      { role: "system", content: "You are a helpful coding assistant." },
      { role: "user", content: "Write a hello world in Python" }
    ]
  })
);

console.log(response.choices[0].message.content);
```

### Configuration Options

```typescript
interface FMConfig {
  /** Server port (default: 11435) */
  port: number;

  /** Path to foundation-bridge binary (auto-detected if not specified) */
  bridgePath?: string;

  /** Request timeout in ms (default: 300000 = 5 minutes) */
  timeoutMs?: number;

  /** Auto-start server if not running (default: true) */
  autoStart?: boolean;
}
```

**Examples:**

```typescript
// Custom port
const client = createFMClient({ port: 8080 });

// Disable auto-start (server must be running)
const client = createFMClient({ autoStart: false });

// Custom timeout (1 minute)
const client = createFMClient({ timeoutMs: 60_000 });

// Custom bridge path
const client = createFMClient({
  bridgePath: "/usr/local/bin/foundation-bridge"
});
```

### Bridge Path Discovery

The client searches for the bridge binary in this order:

1. `FM_BRIDGE_PATH` environment variable
2. `./bin/foundation-bridge` (project root)
3. `./swift/foundation-bridge/.build/release/foundation-bridge`
4. `~/.local/bin/foundation-bridge`
5. `/usr/local/bin/foundation-bridge`
6. `/opt/homebrew/bin/foundation-bridge`

### Health Check

```typescript
import { checkFMHealth, DEFAULT_FM_PORT } from "./src/llm/foundation-models.js";
import { Effect } from "effect";

const health = await Effect.runPromise(
  checkFMHealth(DEFAULT_FM_PORT).pipe(
    Effect.catchAll((e) => Effect.succeed({
      available: false,
      serverRunning: false,
      modelAvailable: false,
      error: e.message
    }))
  )
);

console.log(health);
// { available: true, serverRunning: true, modelAvailable: true, version: "1.0.0" }
```

### Error Handling

```typescript
import { FMError } from "./src/llm/foundation-models.js";

const result = await Effect.runPromise(
  client.chat({ messages: [...] }).pipe(
    Effect.catchAll((error: FMError) => {
      switch (error.reason) {
        case "not_macos":
          return Effect.succeed({ fallback: "Use different provider" });
        case "bridge_not_found":
          return Effect.fail(new Error("Build the bridge first: cd swift/foundation-bridge && ./build.sh"));
        case "server_not_running":
          return Effect.fail(new Error("Server failed to start"));
        case "model_unavailable":
          return Effect.fail(new Error("Enable Apple Intelligence in System Settings"));
        case "timeout":
          return Effect.fail(new Error("Request timed out"));
        default:
          return Effect.fail(error);
      }
    })
  )
);
```

### Error Types

| Reason | Description |
|--------|-------------|
| `not_macos` | Running on non-macOS platform |
| `bridge_not_found` | Cannot find foundation-bridge binary |
| `server_not_running` | Server not running and auto-start failed |
| `model_unavailable` | Foundation Models not available on device |
| `request_failed` | HTTP request failed |
| `invalid_response` | Response missing expected fields |
| `timeout` | Request exceeded timeout |

### Effect Layer

For dependency injection with Effect:

```typescript
import { FMClientTag, fmClientLayer } from "./src/llm/foundation-models.js";
import { Effect, Layer } from "effect";

// Create layer
const layer = fmClientLayer({ port: 11435 });

// Use in program
const program = Effect.gen(function* () {
  const client = yield* FMClientTag;
  return yield* client.chat({
    messages: [{ role: "user", content: "Hello" }]
  });
});

// Run with layer
const result = await Effect.runPromise(
  program.pipe(Effect.provide(layer))
);
```

## TerminalBench Integration

Foundation Models is integrated as a model provider in TerminalBench.

### Model String Format

```bash
# Full format
bun run bench --model fm:apple-foundation-model

# Short forms (all equivalent)
bun run bench --model fm
bun run bench --model foundation-models
bun run bench --model apple
```

### How It Works

The model adapter (`src/bench/model-adapter.ts`) creates an FM runner that:

1. Detects macOS and finds the bridge binary
2. Ensures the server is running
3. Sends requests to `/v1/chat/completions`
4. Parses tool calls from text (FM may not support native JSON tools)

### Tool Calling

Foundation Models may not support native JSON tool calling. The adapter uses text-based tool calling:

```typescript
// System prompt instructs the model to use XML markers:
// <tool_call>{"name":"tool_name","arguments":{...}}</tool_call>

// The adapter parses these from the response text
```

## CLI Demo

The client includes a CLI demo:

```bash
# Health check
bun src/llm/foundation-models.ts --health

# Chat
bun src/llm/foundation-models.ts "What is the meaning of life?"
```

## Troubleshooting

### "Foundation Models requires macOS"

You're running on Linux or Windows. Foundation Models only works on macOS 26+.

### "foundation-bridge binary not found"

Build the bridge:
```bash
cd swift/foundation-bridge
./build.sh
```

Or set the path explicitly:
```bash
export FM_BRIDGE_PATH=/path/to/foundation-bridge
```

### "Model not available on this device"

1. Open **System Settings** → **Apple Intelligence & Siri**
2. Enable Apple Intelligence
3. Wait for model download to complete

### "Server failed to start after 5 seconds"

Check if something else is using port 11435:
```bash
lsof -i :11435
```

Use a different port:
```typescript
const client = createFMClient({ port: 11436 });
```

### Build Error: "'v26' is unavailable"

Update the swift-tools-version in `Package.swift`:
```swift
// swift-tools-version:6.2
```

### Build Error: "'main' attribute cannot be used"

The Package.swift should include:
```swift
swiftSettings: [
    .unsafeFlags(["-parse-as-library"])
]
```

### Slow First Response

The first request may be slower as the model loads into memory. Subsequent requests are faster.

### CORS Errors in Browser

The bridge includes CORS headers for local development. All origins are allowed.

## Performance Notes

- **Latency**: First request ~1-2s (model loading), subsequent ~100-500ms
- **Token estimation**: ~4 characters per token (rough approximation)
- **Memory**: Model uses shared system memory with Apple Intelligence
- **Concurrency**: Sessions are reused within the bridge process

## Comparison with Other Providers

| Feature | Foundation Models | Ollama | OpenRouter |
|---------|------------------|--------|------------|
| Platform | macOS 26+ only | Cross-platform | Cloud |
| Internet | Not required | Not required | Required |
| Privacy | On-device | On-device | Cloud |
| Cost | Free | Free | Pay per token |
| Models | Apple FM only | Many models | Many models |
| Setup | Build bridge | Install app | API key |
| Speed | Fast | Varies | Varies |

## Files Reference

| File | Description |
|------|-------------|
| `swift/foundation-bridge/` | Swift HTTP bridge source |
| `swift/foundation-bridge/build.sh` | Build script |
| `bin/foundation-bridge` | Compiled binary |
| `src/llm/foundation-models.ts` | Bun/TypeScript client |
| `src/llm/foundation-models.test.ts` | Unit tests |
| `src/bench/model-adapter.ts` | TerminalBench integration |

## Future Improvements

- [ ] Streaming support (`stream: true`)
- [ ] Native tool calling (if FM adds support)
- [ ] Image input support (multimodal)
- [ ] launchd service for persistent server
- [ ] Token count from actual FM metrics (not estimated)
