# Issue #4: Implement Token-by-Token ACP Streaming

**Phase:** 2 (Integration)
**Priority:** P0 (Blocking)
**Estimated Effort:** 1 day
**Dependencies:** #2 (Provider Core must be implemented first)
**Related Issues:** #5 (Registration)

---

## Summary

Implement real-time token-by-token streaming from GPTOSS 20B to the UI using ACP `agentMessageChunk` updates, replacing the placeholder single-response implementation from Phase 1.

## Context

Phase 1 implemented basic generation with a single response. This issue adds streaming:
- Tokens appear in real-time as generated
- User sees progress immediately (no waiting for full response)
- Cancel button can stop generation mid-stream
- Following the pattern from `OpenAgentsLocalProvider.fmStream()`

## Acceptance Criteria

- [ ] Token-by-token streaming works (visible character-by-character in UI)
- [ ] First token latency <3 seconds (M2+)
- [ ] Throughput >10 tokens/sec
- [ ] Cancel button stops generation immediately
- [ ] No buffering delays (tokens sent as soon as available)
- [ ] Streaming uses AsyncSequence from ChatSession
- [ ] Harmony channels parsed: `analysis` hidden, `final` shown; `commentary` used for tool call preambles
- [ ] Handles connection loss gracefully
- [ ] Memory remains stable during long generations

## Technical Implementation

**Update GPTOSSModelManager**:

```swift
public actor GPTOSSModelManager {
    /// Stream response token-by-token
    public func streamGenerate(
        prompt: String,
        options: GPTOSSGenerationOptions,
        onToken: @escaping (String) -> Void,
        onComplete: @escaping () -> Void,
        onError: @escaping (Error) -> Void,
        isCancelled: @escaping () -> Bool
    ) async {
        guard let container = modelContainer else {
            onError(GPTOSSError.modelNotLoaded)
            return
        }

        do {
            try await container.perform { model, tokenizer in
                // Harmony compliance: use ChatSession, pass structured messages
                let chat = ChatSession(model)
                let messages: [ChatMessage] = [ .user(prompt) ]
                // Stream tokens; if API supports channel/role deltas, filter out `analysis`
                for try await delta in chat.streamResponse(messages: messages) {
                    if isCancelled() { break }
                    // If delta is typed (channel, content), ignore analysis channel content
                    // Otherwise, forward text as-is; Phase 3: add channel parser
                    onToken(String(describing: delta))
                }
                onComplete()
            }
        } catch {
            onError(error)
        }
    }
}
```

**Update GPTOSSAgentProvider.start()**:

```swift
public func start(
    sessionId: ACPSessionId,
    prompt: String,
    context: AgentContext,
    updateHub: SessionUpdateHub
) async throws -> AgentHandle {
    try await modelManager.loadModel()

    // Stream token-by-token
    await modelManager.streamGenerate(
        prompt: prompt,
        options: GPTOSSGenerationOptions(
            temperature: config.temperature,
            topP: config.topP,
            maxTokens: config.maxTokens
        ),
        onToken: { token in
            Task {
                let chunk = ACP.Client.ContentChunk(content: .text(.init(text: token)))
                await updateHub.sendSessionUpdate(
                    sessionId: sessionId,
                    update: .agentMessageChunk(chunk)
                )
            }
        },
        onComplete: {
            print("[GPTOSS] Generation complete for session: \(sessionId.value)")
        },
        onError: { error in
            print("[GPTOSS] Generation error: \(error)")
        },
        isCancelled: { [weak self] in
            self?.isCancelled(sessionId) ?? true
        }
    )

    return AgentHandle(sessionId: sessionId, mode: id, isStarted: true)
}
```

## Testing

**Manual Test:**
```swift
// In macOS app, start GPTOSS session
let sessionId = ACPSessionId(value: "stream-test")
let updateHub = MockSessionUpdateHub()

let provider = GPTOSSAgentProvider()
let handle = try await provider.start(
    sessionId: sessionId,
    prompt: "Write a comprehensive guide to Swift concurrency with async/await, actors, and task groups. Include code examples.",
    context: AgentContext(...),
    updateHub: updateHub
)

// Observe tokens appearing in real-time
// Click cancel button mid-generation
await provider.cancel(sessionId: sessionId, handle: handle)

// Verify generation stops immediately
```

**Performance Test:**
```swift
func testStreamingPerformance() async throws {
    let provider = GPTOSSAgentProvider()
    let updateHub = MockSessionUpdateHub()

    let start = Date()
    var firstTokenTime: TimeInterval?
    var tokenCount = 0

    await updateHub.onUpdate = { update in
        if case .agentMessageChunk = update {
            tokenCount += 1
            if firstTokenTime == nil {
                firstTokenTime = Date().timeIntervalSince(start)
            }
        }
    }

    _ = try await provider.start(
        sessionId: ACPSessionId(value: "perf-test"),
        prompt: "Generate 500 tokens of text",
        context: AgentContext(...),
        updateHub: updateHub
    )

    let totalTime = Date().timeIntervalSince(start)

    XCTAssertNotNil(firstTokenTime)
    XCTAssertLessThan(firstTokenTime!, 3.0, "First token should arrive <3s")
    XCTAssertGreaterThan(Double(tokenCount) / totalTime, 10.0, "Throughput should be >10 tok/sec")
}
```

## References

- OpenAgentsLocalProvider.fmStream() - streaming pattern
- CodexAgentProvider - JSONL streaming example
- ChatSession MLX docs
- Integration Spec Section 5.1

## Harmony Specifics (Channel & Stop Tokens)

- Do not display `analysis` channel content to end‑users; it contains raw chain‑of‑thought.
- When persisting assistant replies into history, normalize trailing `<|return|>` into `<|end|>` so prior messages are fully formed for the next turn.
- If a tool call is emitted (`<|call|>`), treat the prior CoT as part of the next prompt (include the `analysis` snippet) per harmony guidance; otherwise drop previous CoT.

## Definition of Done

- [ ] Streaming implemented and working
- [ ] First token <3s, throughput >10 tok/sec
- [ ] Cancel stops generation immediately
- [ ] No UI lag or buffering
- [ ] Tests pass
- [ ] Committed with message: "Implement token-by-token streaming for GPTOSS"
