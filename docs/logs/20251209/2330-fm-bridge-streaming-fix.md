# FM Bridge Streaming Fix

## Summary

Fixed FM bridge streaming to work with Commander GPUI application. The old bridge version was running without streaming support, and the Rust client had incorrect type mappings for SSE events.

## Problem

1. **Old bridge running**: An old version of `foundation-bridge` was running that didn't support streaming
2. **Swift build errors**: The Swift bridge code had stale API calls for the FoundationModels framework
3. **Rust client type mismatch**: The `StreamChoice` expected a `text` field but the OpenAI-compatible SSE format uses `delta.content`

## Changes Made

### Swift Foundation Bridge

Fixed multiple files to match current FoundationModels API:

1. **ChatHandler.swift**: Changed `options.samplingMode` to `options.sampling` with correct static methods (`.greedy`, `.random(top:)`, `.random(probabilityThreshold:)`)

2. **StreamHandler.swift**:
   - Fixed streaming to calculate deltas from accumulated content (FoundationModels returns full accumulated text in each snapshot, not deltas)
   - Fixed guided generation to use `snapshot.rawContent.jsonString`
   - Fixed request capture for Sendable compliance

3. **SessionStore.swift**: Simplified to not use non-existent `LanguageModelSession.Message` type

4. **ToolRegistry.swift**: Changed return type to `[ToolDefinition]?` (Tool is a protocol with associated types, can't convert dynamically)

5. **AdapterRegistry.swift**:
   - Changed `Adapter` to `SystemLanguageModel.Adapter`
   - Stubbed `recompileAdapter()` due to Swift 6 Sendable constraints
   - Made `AdapterInfo` conform to `@unchecked Sendable`

6. **Server.swift**: Removed extra `queryString` parameter

### Rust FM Client

Fixed `crates/fm-bridge/src/types.rs`:

```rust
// Before (incorrect):
struct StreamChoice {
    text: String,
    finish_reason: Option<FinishReason>,
}

// After (correct OpenAI format):
struct StreamChoice {
    delta: Option<StreamDelta>,
    finish_reason: Option<FinishReason>,
}

struct StreamDelta {
    content: Option<String>,
    role: Option<String>,
}
```

## Result

Streaming now works end-to-end:
- Swift bridge sends proper SSE events with `delta.content`
- Rust client correctly parses the OpenAI-compatible format
- Commander GPUI app shows streaming text in real-time

## Test Command

```bash
curl -sN http://localhost:3030/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{"model":"apple-foundation-model","messages":[{"role":"user","content":"Say hello"}],"stream":true}'
```

Returns proper SSE chunks with `delta.content` field.
