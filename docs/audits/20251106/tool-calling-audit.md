# Tool Calling Implementation Audit

**Date**: 2025-11-06
**Scope**: Foundation Models tool calling implementation in OpenAgents
**Status**: Analysis complete, improvements recommended

## Executive Summary

We have tool calling infrastructure in place but aren't using it correctly. The FM tools are defined and registered, but the orchestrator generates text plans that we manually parse instead of letting the model invoke tools directly. We also lack persistent sessions, transcript management, and streaming - all critical for multi-turn agent reasoning.

## Current State

### What Works

**Tool Definitions** (ios/OpenAgentsCore/Sources/OpenAgentsCore/Orchestration/FMTools.swift:1-172)
- ✅ All session tools wrapped as FM tools with `@Generable` arguments and `@Guide` constraints
- ✅ Tools registered via `FMToolsRegistry.defaultTools()`
- ✅ Proper argument validation (ranges, oneOf, descriptions)
- ✅ Bounded execution with caps (time, bytes, result counts)
- ✅ Filesystem tools also wrapped (readSpan, grep, listDir)

**Tool Execution** (ios/OpenAgentsCore/Sources/OpenAgentsCore/Orchestration/ToolExecutor.swift:1-169)
- ✅ Type-safe execution layer
- ✅ Progress callbacks for long-running operations
- ✅ Proper error handling with ToolExecutionError enum
- ✅ Safety bounds enforced (MAX_INLINE_BYTES, MAX_INLINE_LINES)

**ACP Integration**
- ✅ Operations mapped to ACP tool calls with proper status tracking
- ✅ Plan entries with in_progress/completed status
- ✅ Tool call updates streamed to iOS via bridge

### What's Broken

**1. We Don't Use FM Tool Calling Loop**

The orchestrator creates a session with tools registered, then asks the model to generate a TEXT plan that we manually parse:

```swift
// ExploreOrchestrator.swift:148-217
let session = LanguageModelSession(model: model, tools: tools, instructions: instructions)
let response = try await session.respond(to: prompt, options: options)
let raw = response.content  // TEXT response
let ops = try parseOperationsFromResponse(raw)  // MANUAL PARSING
```

This defeats the entire purpose of FM tool calling. The model should invoke tools directly via the built-in loop, not generate text that we regex-parse.

**2. No Persistent Session**

A new `LanguageModelSession` is created every time we generate a plan. No transcript, no memory, no multi-turn reasoning.

From stateful-sessions.md:
> Using a persistent LanguageModelSession with prewarming, transcripts, and (optionally) streaming snapshots gives better latency, richer multi-turn reasoning, and stable tool-calling behavior.

We do none of this. Each plan is a cold start.

**3. No Streaming**

We use single-shot `respond()` calls. The model generates the full response, then we get it all at once. No partial updates, no early progress indication.

**4. No Transcript Management**

No memory of prior tool results, no context from previous turns, no token budget tracking. If we wanted the model to refine a plan based on what it learned from executing the first plan, we'd have to manually reconstruct the context.

**5. Text-Based Plan Parsing Is Fragile**

```swift
// ExploreOrchestrator.swift:277-338
private func parseOperationsFromResponse(_ response: String) throws -> [AgentOp] {
    // Extract content from response description
    let content = extractContent(from: response) ?? response
    let lines = content.components(separatedBy: "\n")

    for line in lines {
        let trimmed = line.trimmingCharacters(in: .whitespacesAndNewlines)
        if trimmed.contains("sessionList") || (trimmed.contains("session") && trimmed.contains("list")) {
            if let op = parseSessionList(from: trimmed) { ops.append(op) }
        }
        // ... 7 more brittle string checks
    }
}
```

This is exactly the problem tool calling solves. If the model's text format changes, this breaks. If the model uses synonyms or restructures the output, this breaks. Structured tool calls are type-safe and don't require regex gymnastics.

**6. Manual Parameter Extraction**

```swift
// ExploreOrchestrator.swift:881-1016
private func parseReadSpan(from line: String) -> AgentOp? {
    if let path = extractPath(from: line),
       let (start, end) = extractLineRange(from: line) {
        return AgentOp(kind: .readSpan(ReadSpanParams(path: path, startLine: start, endLine: end)))
    }
    return nil
}

private func extractPath(from line: String) -> String? {
    let pattern = #"([a-zA-Z0-9_\-./]+\.[a-z]+|[a-zA-Z0-9_\-./]+/)"#
    return match(line, pattern: pattern)
}
```

We wrote 130+ lines of manual parsers when the model could just call `FMTool_SessionRead(arguments: .init(sessionId: "...", provider: "..."))` and we'd get type-safe arguments.

## Recommended Improvements

### Priority 1: Use Native FM Tool Calling Loop

**Problem**: We register tools but don't let the model call them.

**Solution**: Remove text plan generation and parsing. Let the model invoke tools directly. Observe tool requests from the session, execute them via ToolExecutor, return results to the session, and let the model decide next steps.

**Sketch**:
```swift
let session = LanguageModelSession(model: model, tools: tools, instructions: instructions)

let initialPrompt = """
Workspace: \(workspaceName)
Goals: \(goalsStr)

Use the available tools to explore the workspace and achieve these goals.
"""

var toolResults: [String: Any] = [:]

// Multi-turn loop
while true {
    let response = try await session.respond(to: prompt)

    // Check if model wants to call a tool
    if let toolRequest = response.toolRequest {
        // Execute tool via ToolExecutor
        let result = try await toolExecutor.execute(toolRequest.toolName, arguments: toolRequest.arguments)

        // Return result to model
        try await session.addToolResult(toolRequest.id, result: result)

        // Continue loop
        continue
    }

    // No more tool calls - model is done
    break
}
```

This eliminates all manual parsing, makes the flow robust to model output changes, and enables multi-step reasoning where the model refines its plan based on actual results.

### Priority 2: Implement Persistent Session with Transcript

**Problem**: No memory between orchestration runs.

**Solution**: Keep a single `LanguageModelSession` instance in `ExploreOrchestrator` actor. Maintain transcript across multiple `startExploration()` calls.

**Changes**:
```swift
@available(iOS 26.0, macOS 26.0, *)
public actor ExploreOrchestrator {
    private let workspaceRoot: String
    private let policy: ExplorationPolicy
    private let streamHandler: ACPUpdateStreamHandler

    // Persistent FM session
    private var fmSession: LanguageModelSession?
    private var fmTranscript: Transcript?

    public init(...) {
        // ...
        self.fmSession = nil  // Lazy init on first use
    }

    private func getOrCreateSession() async throws -> LanguageModelSession {
        if let session = fmSession {
            return session
        }

        let tools = FMToolsRegistry.defaultTools(workspaceRoot: workspaceRoot)
        let instructions = Instructions("You are a workspace exploration assistant...")
        let session = LanguageModelSession(
            model: SystemLanguageModel.default,
            tools: tools,
            instructions: instructions
        )
        try? session.prewarm(promptPrefix: nil)
        fmSession = session
        return session
    }

    public func startExploration() async throws -> ExploreSummary {
        let session = try await getOrCreateSession()

        // Use session with transcript memory
        // Multi-turn loop here

        return summary
    }
}
```

This gives the model context from prior runs, enabling it to refine its strategy based on what it learned.

### Priority 3: Add Streaming Support

**Problem**: No progress indication during FM generation.

**Solution**: Use `stream` instead of `respond` for long operations. Forward snapshots as ACP `agent_message_chunk` updates.

```swift
let stream = try await session.stream(prompt, options: options)

for try await snapshot in stream {
    // Forward partial content to iOS
    let chunk = ACP.Client.ContentChunk(
        content: .text(.init(text: snapshot.content))
    )
    await streamHandler(.agentMessageChunk(chunk))
}
```

This keeps the UI responsive and gives users confidence that work is happening.

### Priority 4: Token Budget Management

**Problem**: No transcript pruning, will hit 4096 token limit on long sessions.

**Solution**: Track estimated tokens, summarize old turns when approaching limit.

```swift
private func ensureTokenBudget(_ session: LanguageModelSession) async throws {
    guard let transcript = fmTranscript else { return }

    let estimatedTokens = estimateTokens(transcript)
    guard estimatedTokens > 3500 else { return }

    // Summarize older turns
    let summary = try await summarizeOldTurns(session, transcript: transcript)

    // Build new transcript with summary + recent turns
    fmTranscript = buildSlidingWindowTranscript(summary: summary, recentTurns: 5)
}
```

### Priority 5: Richer Tool Outputs

**Problem**: FM tools return plain strings. This loses structured information.

**Solution**: Return structured `@Generable` outputs that the model can reason about more effectively.

```swift
@available(iOS 26.0, macOS 26.0, *)
struct FMTool_SessionSearch: Tool {
    let name = "session.search"
    let description = "Search conversation history for a regex pattern."

    @Generable
    struct Arguments { /* ... */ }

    // Return structured output instead of String
    @Generable
    struct Output {
        let totalMatches: Int
        let truncated: Bool
        let sampleMatches: [Match]

        struct Match: Codable {
            let sessionId: String
            let lineNumber: Int
            let snippet: String
        }
    }

    func call(arguments a: Arguments) async throws -> Output {
        let tool = SessionSearchTool()
        let res = try await tool.search(...)
        return Output(
            totalMatches: res.totalMatches,
            truncated: res.truncated,
            sampleMatches: res.matches.prefix(3).map { ... }
        )
    }
}
```

The model can then make decisions based on `output.totalMatches` instead of parsing strings.

## Implementation Roadmap

### Phase 1: Native Tool Calling (1-2 days)

1. Remove `parseOperationsFromResponse` and all manual parsers
2. Implement multi-turn tool calling loop in `ExploreOrchestrator`
3. Map FM tool requests to `ToolExecutor` operations
4. Return tool results to session for next turn
5. Add tests for tool calling loop

**Risk**: Low. This simplifies the code and uses Apple's tested API.

### Phase 2: Persistent Session (1 day)

1. Add `fmSession` and `fmTranscript` to orchestrator actor
2. Lazy init with prewarming
3. Maintain transcript across calls
4. Add tests for multi-turn memory

**Risk**: Low. Standard FM pattern.

### Phase 3: Streaming (1 day)

1. Replace `respond()` with `stream()` in plan/analysis generation
2. Forward snapshots to ACP stream handler
3. Add tests for partial updates

**Risk**: Low. FM streaming is stable.

### Phase 4: Token Management (2 days)

1. Implement token estimation
2. Add transcript summarization
3. Build sliding window transcript
4. Add tests for budget enforcement

**Risk**: Medium. Summarization quality depends on prompt design.

### Phase 5: Structured Outputs (1 day)

1. Define `@Generable` output types for all FM tools
2. Update tool implementations
3. Test structured output parsing

**Risk**: Low. Already using `@Generable` for arguments.

## Testing Strategy

### Unit Tests

- Tool calling loop with mocked FM responses
- Transcript management (append, summarize, prune)
- Token budget enforcement
- Structured output parsing

### Integration Tests

- End-to-end orchestration with real FM session
- Multi-turn tool calling with tool results
- Streaming partial updates
- Session persistence across multiple runs

### Smoke Tests

- Orchestrator works when FM unavailable (fallback logic)
- Large workspace exploration stays under token limit
- Tool call errors don't crash orchestration loop

## Migration Path

We can't break the current orchestration flow. Recommended strategy:

1. **Keep existing text-plan path as fallback**
   - If native tool calling fails, fall back to current text parsing
   - Log which path was used for diagnostics

2. **Feature flag for new flow**
   - Add `policy.use_native_tool_calling: Bool`
   - Default to `false` initially, flip to `true` after validation

3. **Gradual rollout**
   - Enable native tool calling for internal testing
   - Monitor ACP streams for correct behavior
   - Remove text-plan path once native tool calling is proven

## Conclusion

We built the foundation (tool definitions, registration, execution) but aren't using it correctly. Switching to native FM tool calling eliminates 400+ lines of brittle parsing code, enables multi-turn reasoning, and makes the system robust to model output changes.

The path forward is clear: let the model call tools directly, maintain session state across turns, stream partial updates, and manage token budget. None of this is risky - these are standard FM patterns documented by Apple.

## References

- `docs/foundation-models/tool-calling.md` - Tool implementation guide
- `docs/foundation-models/stateful-sessions.md` - Session management guide
- `docs/adr/0006-foundation-models.md` - ADR for FM adoption
- `ios/OpenAgentsCore/Sources/OpenAgentsCore/Orchestration/FMTools.swift` - Current tool definitions
- `ios/OpenAgentsCore/Sources/OpenAgentsCore/Orchestration/ExploreOrchestrator.swift` - Current orchestrator implementation
