#+ Streaming and Snapshots (Apple Foundation Models)

Foundation Models streams snapshots (partially populated objects) rather than raw token deltas. This makes SwiftUI bindings straightforward for both plain text and structured results.

## Why Snapshots

- Receive complete, partially filled objects; properties become non‑nil over time
- Bind directly to view state; avoid manual accumulation/JSON partial parsing
- Works for text and `@Generable` types

## Plain Text Streaming

```swift
let session = LanguageModelSession()
let stream = session.streamResponse(to: prompt)

for try await chunk in stream {
    // Append chunk.content to your UI
}
let final = try await stream.collect()
```

## Structured Streaming

```swift
@Generable struct Answer { @Guide(description: "Short answer") var text: String }
let stream = session.streamResponse(to: prompt, generating: Answer.self)
for try await snapshot in stream {
    // snapshot.content?.text becomes non‑nil progressively
}
```

## Cancellation & Backpressure

- Cancel the enclosing task to stop an in‑flight stream
- Serialize per‑session requests; avoid concurrent `respond/stream` on the same session (handle `concurrentRequests` errors)
- Prefer a single active stream per session; queue follow‑ups

## UI Patterns

- Observe `session.transcript` for chat UIs; snapshots will update entries as they fill
- Use stable IDs for lists to prevent row recycling/jitter during updates
- Animate field appearance for structured content (e.g., placeholders → values)

## Error Handling

- `guardrailViolation`: present a friendly, human explanation; offer to rephrase
- `exceededContextWindowSize`: trim/sliding window or summarize transcript
- `assetsUnavailable`: show availability UI; fall back if needed

## OpenAgents Guidance

- For longer responses (analysis, summaries), prefer streaming. Forward partials via ACP as `agent_message_chunk` updates.
- Keep one stream per session; ExploreOrchestrator (actor) should serialize calls to prevent `concurrentRequests`.
- Consider prewarming before user actions to reduce time‑to‑first‑chunk.

