# Foundation Model Bridge Setup & Debugging

**Last Updated:** 2025-12-09

## Overview

The Foundation Model bridge (`foundation-bridge`) is a Swift HTTP server that exposes Apple's on-device Foundation Models API via an OpenAI-compatible REST API.

**Architecture:**
```
┌─────────────┐      HTTP       ┌──────────────────┐      Swift API      ┌─────────────────────┐
│ Rust Client │ ────────────────> │ Swift HTTP Bridge│ ──────────────────> │ Apple Foundation    │
│  (fm-bridge)│ <──────────────── │ (foundation-     │ <─────────────────── │ Models (on-device)  │
└─────────────┘   JSON Response  │  bridge)         │    LLM Response     └─────────────────────┘
```

## Files

**Swift Bridge:**
- `swift/foundation-bridge/Sources/foundation-bridge/main.swift` - Entry point
- `swift/foundation-bridge/Sources/foundation-bridge/Server.swift` - HTTP server using Network.framework
- `swift/foundation-bridge/Sources/foundation-bridge/ChatHandler.swift` - FoundationModels integration
- `swift/foundation-bridge/Sources/foundation-bridge/Types.swift` - OpenAI-compatible request/response types
- `swift/foundation-bridge/Sources/foundation-bridge/GuidedTypes.swift` - Structured generation schemas
- `swift/foundation-bridge/build.sh` - Build script

**Rust Client:**
- `fm-bridge/src/lib.rs` - Public API
- `fm-bridge/src/client.rs` - HTTP client
- `fm-bridge/src/types.rs` - Request/response types
- `fm-bridge/src/error.rs` - Error handling
- `fm-bridge/src/bin/cli.rs` - CLI tool

## API Endpoints

The Swift bridge exposes:

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/health` | Health check + model availability |
| GET | `/v1/models` | List available models |
| POST | `/v1/chat/completions` | Chat completion (OpenAI-compatible) |

**Important:** The Swift bridge uses `/v1/chat/completions` (chat API), NOT `/v1/completions` (legacy completions API).

## Building & Running

### Swift Bridge

**Build:**
```bash
bun run bridge:build
```

This runs `swift/foundation-bridge/build.sh` which:
1. Builds with `swift build -c release`
2. Creates wrapper script at `bin/foundation-bridge` that calls the binary from build directory
3. Makes wrapper executable

**Run:**
```bash
# Via bun script (RECOMMENDED)
bun run bridge

# Or directly with custom port
bin/foundation-bridge 3030

# Or from build directory
swift/foundation-bridge/.build/release/foundation-bridge [port]
```

**Default port:** 11435

### Rust Client

**Build:**
```bash
cd fm-bridge
cargo build --release
```

**Install CLI:**
```bash
cd fm-bridge
cargo install --path .
```

**Usage:**
```bash
# Health check
fm health

# List models
fm models

# Complete a prompt
fm complete "Hello, world!"

# With streaming
fm complete "Count to 10" --stream

# Custom server URL
fm --url http://localhost:11435 complete "Hi"
```

## Known Issues & Fixes

### Issue 1: SIGKILL When Running from bin/ [FIXED ✅]

**Symptom:**
```bash
$ bin/foundation-bridge
error: script "bridge" was terminated by signal SIGKILL
[1]    75082 killed     bun run bridge
```

**Root Cause:**
- The binary loses entitlements when copied from `.build/release/` to `bin/`
- macOS requires specific entitlements to access FoundationModels framework
- Ad-hoc code signature is insufficient when binary is copied

**Fix Applied:**
Updated `build.sh` to create a wrapper script instead of copying the binary. The wrapper script (`bin/foundation-bridge`) executes the binary from its build directory location, preserving its code signature and entitlements.

**How it works:**
```bash
# bin/foundation-bridge is now a bash wrapper script that calls:
$REPO_ROOT/swift/foundation-bridge/.build/release/foundation-bridge "$@"
```

**Benefits:**
- Binary stays in build directory with valid signature
- Wrapper script provides same UX as direct binary
- `bun run bridge` now works correctly
- No need for manual re-signing or entitlements files

**Testing:**
```bash
$ bun run bridge:build  # Creates wrapper
$ bun run bridge        # Now works!
$ bin/foundation-bridge 3030  # Also works!
```

### Issue 2: Swift Continuation Warning

**Symptom:**
```
SWIFT TASK CONTINUATION MISUSE: start() leaked its continuation without resuming it.
This may cause tasks waiting on it to remain suspended forever.
```

**Location:** `Server.swift:46` - The `withCheckedContinuation` in `start()` method

**Current Code:**
```swift
await withCheckedContinuation { (_: CheckedContinuation<Void, Never>) in
    // Never resume - keeps the task alive forever
    RunLoop.main.run()
}
```

**Impact:** Warning only, server runs fine

**Root Cause:**
- Using continuation without resuming to keep server alive
- This is intentional but triggers Swift runtime warning

**Proper Fix (TODO):**
Replace with a proper async infinite loop:
```swift
// Keep server running indefinitely
try await withThrowingTaskGroup(of: Void.self) { group in
    group.addTask {
        // This task never completes, keeping the server alive
        try await Task.sleep(for: .seconds(.max))
    }
    try await group.next()
}
```

### Issue 3: API Mismatch (Rust client vs Swift bridge) [FIXED ✅]

**Symptom:**
```
Error: ApiError { status: 404, message: "Not found: POST /v1/completions" }
```

**Root Cause:**
- Rust client was calling `/v1/completions` (legacy API)
- Swift bridge exposes `/v1/chat/completions` (chat API)
- API mismatch

**Fix Applied:**
Updated Rust client to use OpenAI-compatible chat completions API:
1. Changed request format from `{prompt, model}` to `{messages: [{role, content}]}`
2. Updated endpoint from `/v1/completions` to `/v1/chat/completions`
3. Updated response parsing to handle chat format with `message.content`
4. Updated ModelInfo types to match Swift bridge response format

**Verification:**
```bash
$ fm --url http://localhost:3030 health
✓ API is healthy

$ fm --url http://localhost:3030 models
Available models:
  - apple-foundation-model
    Owner: apple

$ fm --url http://localhost:3030 complete "What is 2+2?"
Response:
---
2 + 2 equals 4.
---
Usage: 7 prompt + 3 completion = 10 total tokens
```

**All endpoints working:**
- ✅ Health check
- ✅ Models listing
- ✅ Chat completions
- ⚠️  Streaming (not yet implemented in Swift bridge)

## Testing

### Test Swift Bridge

```bash
# Start bridge
swift/foundation-bridge/.build/release/foundation-bridge 3030

# Health check
curl http://localhost:3030/health

# Expected:
# {
#   "status": "ok",
#   "model_available": true,
#   "version": "1.0.0",
#   "platform": "macOS"
# }

# List models
curl http://localhost:3030/v1/models

# Chat completion
curl http://localhost:3030/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{
    "model": "apple-foundation-model",
    "messages": [{"role": "user", "content": "Hello!"}]
  }'
```

### Test Rust Client

```bash
# Requires bridge running on localhost:3030
fm --url http://localhost:3030 health
fm --url http://localhost:3030 models
fm --url http://localhost:3030 complete "Hi"
```

## Architecture Details

### Swift Bridge Request Flow

1. **Server.swift** receives HTTP request on port
2. **Server.swift** parses HTTP, extracts JSON body
3. **Server.swift** routes to `handleChatCompletions()`
4. **Server.swift** deserializes JSON to `ChatCompletionRequest`
5. **ChatHandler.swift** validates model availability
6. **ChatHandler.swift** builds prompt from messages
7. **ChatHandler.swift** calls `LanguageModelSession.respond()`
8. **ChatHandler.swift** wraps response in OpenAI format
9. **Server.swift** serializes to JSON and sends HTTP response

### Rust Client Request Flow

1. **CLI** parses args, builds `CompletionOptions`
2. **Client** creates `CompletionRequest` with `messages: [{role: "user", content: prompt}]`
3. **Client** POST to `/v1/chat/completions`
4. **Client** deserializes `CompletionResponse`
5. **CLI** extracts `response.choices[0].message.content` and prints result

## Next Steps

1. ✅ ~~Fix Rust client to use `/v1/chat/completions` endpoint~~ - DONE
2. ✅ ~~Fix SIGKILL issue with wrapper script~~ - DONE
3. Add streaming support to Swift bridge (SSE implementation needed)
4. Fix Swift continuation warning with proper async pattern
5. Add integration tests
6. Add comprehensive error handling and logging

## References

- [FoundationModels Framework Docs](https://developer.apple.com/documentation/foundationmodels)
- [OpenAI Chat Completions API](https://platform.openai.com/docs/api-reference/chat)
- [macOS Code Signing Guide](https://developer.apple.com/documentation/xcode/code-signing-guide)
