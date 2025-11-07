# Tool Calling Implementation Guide

**Companion to**: tool-calling-audit.md
**For**: Coding agents and developers implementing the improvements
**Updated**: 2025-11-06

## Overview

This guide provides specific, actionable steps to fix the tool calling implementation. Each section includes code snippets, file locations, and acceptance criteria.

## Quick Reference

**Files to modify**:
- `ios/OpenAgentsCore/Sources/OpenAgentsCore/Orchestration/ExploreOrchestrator.swift`
- `ios/OpenAgentsCore/Sources/OpenAgentsCore/Orchestration/FMTools.swift`
- `ios/OpenAgentsCore/Tests/OpenAgentsCoreTests/ExploreOrchestratorTests.swift` (new)

**Files to remove** (after migration):
- Lines 277-1016 in ExploreOrchestrator.swift (all manual parsers)

## Implementation Tasks

### Task 1: Add Persistent Session to ExploreOrchestrator

**File**: `ios/OpenAgentsCore/Sources/OpenAgentsCore/Orchestration/ExploreOrchestrator.swift`

**Current state** (lines 55-100):
```swift
@available(iOS 26.0, macOS 26.0, *)
public actor ExploreOrchestrator {
    private let workspaceRoot: String
    private let goals: [String]
    private let policy: ExplorationPolicy
    private var currentPlan: ExplorePlan?
    private var currentACPPlan: ACPPlan?
    // ...
}
```

**New state**:
```swift
@available(iOS 26.0, macOS 26.0, *)
public actor ExploreOrchestrator {
    private let workspaceRoot: String
    private let goals: [String]
    private let policy: ExplorationPolicy

    // Persistent FM session and transcript
    private var fmSession: LanguageModelSession?
    private var fmTranscript: Transcript?
    private var tokenEstimate: Int = 0

    private var currentPlan: ExplorePlan?
    private var currentACPPlan: ACPPlan?
    // ... rest unchanged
}
```

**Add helper method**:
```swift
@available(iOS 26.0, macOS 26.0, *)
private func getOrCreateSession() async throws -> LanguageModelSession {
    if let existing = fmSession {
        return existing
    }

    let tools = FMToolsRegistry.defaultTools(workspaceRoot: workspaceRoot)
    let instructions = Instructions("""
    You are a workspace exploration assistant. Use the available tools to explore the workspace and achieve the user's goals.

    Available tools:
    - session.list: List recent conversation sessions
    - session.search: Search sessions for patterns
    - session.read: Read session content
    - session.analyze: Analyze sessions for insights
    - content.get_span: Read file content
    - code.grep: Search code
    - fs.list_dir: List directory contents

    After using tools, summarize your findings and suggest next steps.
    """)

    let session: LanguageModelSession
    if let transcript = fmTranscript {
        session = LanguageModelSession(
            model: SystemLanguageModel.default,
            tools: tools,
            instructions: instructions,
            transcript: transcript
        )
    } else {
        session = LanguageModelSession(
            model: SystemLanguageModel.default,
            tools: tools,
            instructions: instructions
        )
    }

    try? session.prewarm(promptPrefix: nil)
    fmSession = session
    print("[Orchestrator] FM session created with \(tools.count) tools")
    return session
}
```

**Acceptance criteria**:
- [ ] Session persists across multiple `startExploration()` calls
- [ ] Transcript carries context from prior runs
- [ ] Prewarming happens once per session creation

---

### Task 2: Implement Native Tool Calling Loop

**File**: `ios/OpenAgentsCore/Sources/OpenAgentsCore/Orchestration/ExploreOrchestrator.swift`

**Replace** `generateInitialPlan()` (lines 148-217) with:

```swift
@available(iOS 26.0, macOS 26.0, *)
private func executeToolCallingLoop(goals: [String]) async throws -> [ToolCallResult] {
    let session = try await getOrCreateSession()

    let workspaceName = (workspaceRoot as NSString).lastPathComponent
    let goalsStr = goals.isEmpty ? "(explore the workspace)" : goals.joined(separator: "\n- ")

    let prompt = """
    Workspace: \(workspaceName)
    Goals:
    - \(goalsStr)

    Use the available tools to explore the workspace and achieve these goals. Start with session.analyze to understand recent work, then use other tools as needed.
    """

    var results: [ToolCallResult] = []
    var turnCount = 0
    let maxTurns = 10  // Safety limit

    print("[Orchestrator] Starting tool calling loop with goals: \(goalsStr)")

    // Initial prompt
    var currentPrompt: String? = prompt

    while turnCount < maxTurns {
        turnCount += 1
        print("[Orchestrator] Turn \(turnCount)/\(maxTurns)")

        // Check token budget before each turn
        try await ensureTokenBudget(session)

        let response: LanguageModelSession.Response<String>
        if let p = currentPrompt {
            response = try await session.respond(to: p)
            currentPrompt = nil  // Only use initial prompt once
        } else {
            // Continue conversation (model will use transcript context)
            response = try await session.respond(to: "Continue with the next step.")
        }

        // Check for tool calls
        // NOTE: LanguageModelSession API for tool calls may vary by SDK version
        // This is conceptual - adjust based on actual Foundation Models API
        if let toolRequest = response.toolRequest {
            print("[Orchestrator] Model requested tool: \(toolRequest.toolName)")

            // Stream tool call start
            let op = AgentOp(
                kind: opKindFromToolRequest(toolRequest),
                priority: 0
            )
            await streamToolCall(op, status: .started)

            // Execute tool
            let result: any Encodable
            do {
                result = try await toolExecutor.execute(op)
                await streamToolCallUpdate(op, status: .completed, output: result)
                print("[Orchestrator] Tool completed: \(toolRequest.toolName)")
            } catch {
                await streamToolCallUpdate(op, status: .error, error: error.localizedDescription)
                print("[Orchestrator] Tool failed: \(error)")
                throw error
            }

            results.append(ToolCallResult(
                toolName: toolRequest.toolName,
                arguments: toolRequest.arguments,
                result: result
            ))

            // Return result to model
            // NOTE: API varies - adjust based on actual Foundation Models API
            try await session.addToolResult(toolRequest.id, result: result)

            // Continue loop
            continue
        }

        // No tool call - model provided final response
        print("[Orchestrator] Model finished. Final response: \(response.content.prefix(200))")
        break
    }

    if turnCount >= maxTurns {
        print("[Orchestrator] Hit turn limit (\(maxTurns))")
    }

    return results
}

struct ToolCallResult {
    let toolName: String
    let arguments: Any
    let result: any Encodable
}

private func opKindFromToolRequest(_ request: ToolRequest) -> AgentOpKind {
    // Map tool request to AgentOpKind
    // This is a simplified example - expand based on actual tool arguments
    switch request.toolName {
    case "session.list":
        return .sessionList(SessionListParams(provider: nil, topK: 20))
    case "session.search":
        return .sessionSearch(SessionSearchParams(pattern: "", provider: nil))
    case "session.read":
        return .sessionRead(SessionReadParams(sessionId: "", provider: "claude-code"))
    case "session.analyze":
        return .sessionAnalyze(SessionAnalyzeParams(sessionIds: [], provider: nil))
    case "content.get_span":
        return .readSpan(ReadSpanParams(path: "", startLine: 1, endLine: 100))
    case "code.grep":
        return .grep(GrepParams(pattern: "", pathPrefix: nil))
    case "fs.list_dir":
        return .listDir(ListDirParams(path: ".", depth: 0))
    default:
        fatalError("Unknown tool: \(request.toolName)")
    }
}
```

**Notes**:
- The actual Foundation Models API for tool calling may differ from this sketch
- Check Apple's documentation for the correct API to:
  - Detect tool call requests in responses
  - Extract tool name and arguments
  - Return tool results to the session
- Adjust `ToolRequest` type based on actual SDK

**Acceptance criteria**:
- [ ] Model invokes tools directly (no manual parsing)
- [ ] Tool results returned to model for next turn
- [ ] Loop continues until model indicates completion
- [ ] Turn limit prevents infinite loops
- [ ] ACP updates streamed for each tool call

---

### Task 3: Add Token Budget Management

**File**: `ios/OpenAgentsCore/Sources/OpenAgentsCore/Orchestration/ExploreOrchestrator.swift`

**Add these methods**:

```swift
@available(iOS 26.0, macOS 26.0, *)
private func ensureTokenBudget(_ session: LanguageModelSession) async throws {
    guard let transcript = fmTranscript else {
        tokenEstimate = 0
        return
    }

    // Estimate tokens (4 chars per token rule of thumb)
    let transcriptText = String(describing: transcript)
    tokenEstimate = transcriptText.count / 4

    print("[Orchestrator] Estimated tokens: \(tokenEstimate)/4096")

    guard tokenEstimate > 3500 else {
        // Under budget
        return
    }

    print("[Orchestrator] Approaching token limit, summarizing old turns...")

    // Summarize older turns
    let summary = try await summarizeOldTurns(session, transcript: transcript)

    // Build sliding window transcript
    fmTranscript = buildSlidingWindowTranscript(summary: summary, recentTurns: 5)

    // Reset session with new transcript
    fmSession = nil
    _ = try await getOrCreateSession()
}

@available(iOS 26.0, macOS 26.0, *)
private func summarizeOldTurns(_ session: LanguageModelSession, transcript: Transcript) async throws -> String {
    // Use FM to summarize old turns
    let prompt = """
    Summarize the following conversation history in 2-3 sentences, focusing on:
    - Key findings from tool calls
    - Files/sessions explored
    - Main insights

    History: \(String(describing: transcript).prefix(2000))
    """

    let response = try await session.respond(to: prompt)
    return response.content
}

@available(iOS 26.0, macOS 26.0, *)
private func buildSlidingWindowTranscript(summary: String, recentTurns: Int) -> Transcript {
    // Build new transcript with summary + recent turns
    // NOTE: Actual Transcript API varies - adjust based on Foundation Models SDK
    var newTranscript = Transcript()

    // Add summary as system message
    newTranscript.append(role: .system, content: "Previous conversation summary: \(summary)")

    // Add recent turns from old transcript
    if let old = fmTranscript {
        let recent = old.entries.suffix(recentTurns)
        for entry in recent {
            newTranscript.append(entry)
        }
    }

    return newTranscript
}
```

**Acceptance criteria**:
- [ ] Token estimate tracked per turn
- [ ] Summarization triggered when approaching 4096 limit
- [ ] Sliding window preserves recent turns + summary
- [ ] Session continues after transcript pruning

---

### Task 4: Add Streaming Support

**File**: `ios/OpenAgentsCore/Sources/OpenAgentsCore/Orchestration/ExploreOrchestrator.swift`

**Add streaming method**:

```swift
@available(iOS 26.0, macOS 26.0, *)
private func streamToolCallingLoop(goals: [String]) async throws -> [ToolCallResult] {
    let session = try await getOrCreateSession()

    let workspaceName = (workspaceRoot as NSString).lastPathComponent
    let goalsStr = goals.isEmpty ? "(explore the workspace)" : goals.joined(separator: "\n- ")

    let prompt = """
    Workspace: \(workspaceName)
    Goals:
    - \(goalsStr)

    Use the available tools to explore the workspace and achieve these goals.
    """

    var results: [ToolCallResult] = []
    var turnCount = 0
    let maxTurns = 10

    print("[Orchestrator] Starting streaming tool calling loop")

    var currentPrompt: String? = prompt

    while turnCount < maxTurns {
        turnCount += 1

        try await ensureTokenBudget(session)

        // Use stream instead of respond
        let stream: AsyncThrowingStream<LanguageModelSession.Snapshot, Error>
        if let p = currentPrompt {
            stream = try await session.stream(p)
            currentPrompt = nil
        } else {
            stream = try await session.stream("Continue with the next step.")
        }

        var lastSnapshot: LanguageModelSession.Snapshot?

        for try await snapshot in stream {
            lastSnapshot = snapshot

            // Forward partial content to iOS
            if !snapshot.content.isEmpty {
                let chunk = ACP.Client.ContentChunk(
                    content: .text(.init(text: snapshot.content))
                )
                await streamHandler(.agentMessageChunk(chunk))
            }
        }

        guard let finalSnapshot = lastSnapshot else {
            throw OrchestrationError.executionFailed("No response from model")
        }

        // Check for tool call in final snapshot
        if let toolRequest = finalSnapshot.toolRequest {
            // Execute tool (same as non-streaming version)
            let op = AgentOp(kind: opKindFromToolRequest(toolRequest), priority: 0)
            await streamToolCall(op, status: .started)

            let result = try await toolExecutor.execute(op)
            await streamToolCallUpdate(op, status: .completed, output: result)

            results.append(ToolCallResult(
                toolName: toolRequest.toolName,
                arguments: toolRequest.arguments,
                result: result
            ))

            try await session.addToolResult(toolRequest.id, result: result)
            continue
        }

        // No tool call - model finished
        break
    }

    return results
}
```

**Acceptance criteria**:
- [ ] Partial content streamed to iOS during generation
- [ ] ACP agent_message_chunk updates sent
- [ ] Final snapshot processed for tool calls
- [ ] No regression in tool calling behavior

---

### Task 5: Update FMTools to Return Structured Outputs

**File**: `ios/OpenAgentsCore/Sources/OpenAgentsCore/Orchestration/FMTools.swift`

**Example for session.search** (lines 48-63):

**Current**:
```swift
@available(iOS 26.0, macOS 26.0, *)
struct FMTool_SessionSearch: Tool {
    let name = "session.search"
    let description = "Search session history for a regex pattern with limited context (bounded)."
    typealias Output = String  // ❌ Plain string

    @Generable
    struct Arguments { /* ... */ }

    func call(arguments a: Arguments) async throws -> Output {
        let tool = SessionSearchTool()
        let res = try await tool.search(...)
        let sample = res.matches.prefix(3).map { "\($0.sessionId)#\($0.lineNumber)" }.joined(separator: ", ")
        return "session.search matches=\(res.totalMatches) truncated=\(res.truncated) sample=[\(sample)]"
    }
}
```

**Improved**:
```swift
@available(iOS 26.0, macOS 26.0, *)
struct FMTool_SessionSearch: Tool {
    let name = "session.search"
    let description = "Search session history for a regex pattern with limited context (bounded)."

    @Generable
    struct Arguments { /* ... unchanged ... */ }

    // ✅ Structured output
    @Generable
    struct Output {
        let totalMatches: Int
        let truncated: Bool
        let sampleMatches: [Match]

        struct Match: Codable {
            let sessionId: String
            let provider: String
            let lineNumber: Int
            let snippet: String
        }
    }

    func call(arguments a: Arguments) async throws -> Output {
        let tool = SessionSearchTool()
        let res = try await tool.search(
            pattern: a.pattern,
            provider: a.provider,
            sessionIds: a.sessionIds,
            maxResults: a.maxResults,
            contextLines: a.contextLines
        )

        let samples = res.matches.prefix(3).map { m in
            Output.Match(
                sessionId: m.sessionId,
                provider: m.provider,
                lineNumber: m.lineNumber,
                snippet: String(m.line.prefix(100))
            )
        }

        return Output(
            totalMatches: res.totalMatches,
            truncated: res.truncated,
            sampleMatches: samples
        )
    }
}
```

**Repeat for**:
- `FMTool_SessionList` → return structured list with metadata
- `FMTool_SessionRead` → return structured events
- `FMTool_SessionAnalyze` → return structured metrics
- `FMTool_ReadSpan` → return structured content
- `FMTool_Grep` → return structured matches

**Acceptance criteria**:
- [ ] All FM tools return `@Generable` structured outputs
- [ ] Model can access fields directly (e.g., `output.totalMatches`)
- [ ] No manual string parsing in tool implementations

---

### Task 6: Add Feature Flag for Gradual Rollout

**File**: `ios/OpenAgentsCore/Sources/OpenAgentsCore/Orchestration/OrchestrationTypes.swift`

**Update ExplorationPolicy** (lines 40-52):

```swift
public struct ExplorationPolicy: Codable, Sendable {
    /// Allow external LLM calls (Phase 2: always false, on-device only)
    public var allow_external_llms: Bool

    /// Allow network access
    public var allow_network: Bool

    /// Use native FM tool calling loop (vs text plan parsing)
    public var use_native_tool_calling: Bool

    public init(
        allow_external_llms: Bool = false,
        allow_network: Bool = false,
        use_native_tool_calling: Bool = false  // Default to false for safety
    ) {
        self.allow_external_llms = allow_external_llms
        self.allow_network = allow_network
        self.use_native_tool_calling = use_native_tool_calling
    }
}
```

**Update ExploreOrchestrator.startExploration()**:

```swift
public func startExploration() async throws -> ExploreSummary {
    // ... availability checks ...

    if policy.use_native_tool_calling {
        print("[Orchestrator] Using native FM tool calling loop")
        let results = try await executeToolCallingLoop(goals: goals)
        return try await generateSummaryFromToolResults(results)
    } else {
        print("[Orchestrator] Using legacy text plan parsing (fallback)")
        // Keep existing generateInitialPlan() logic
        let plan = try await generateInitialPlan(using: model)
        currentPlan = plan
        await streamPlan(plan)
        try await executeOperations(plan.nextOps)
        return try await generateSummary()
    }
}
```

**Acceptance criteria**:
- [ ] Both paths work independently
- [ ] Policy flag controls which path is used
- [ ] Default is legacy path (no breaking changes)
- [ ] Can A/B test native tool calling safely

---

### Task 7: Add Tests

**New file**: `ios/OpenAgentsCore/Tests/OpenAgentsCoreTests/ExploreOrchestratorTests.swift`

```swift
import XCTest
@testable import OpenAgentsCore

#if canImport(FoundationModels)
import FoundationModels

@available(iOS 26.0, macOS 26.0, *)
final class ExploreOrchestratorTests: XCTestCase {

    func testPersistentSession() async throws {
        // Create orchestrator
        let orchestrator = ExploreOrchestrator(
            workspaceRoot: "/tmp/test",
            goals: ["Test goal"],
            policy: ExplorationPolicy(),
            streamHandler: { _ in }
        )

        // First exploration
        _ = try await orchestrator.startExploration()

        // Session should persist
        // Second exploration should reuse session
        _ = try await orchestrator.startExploration()

        // TODO: Assert session was reused (requires exposing session ID or transcript length)
    }

    func testTokenBudgetEnforcement() async throws {
        // Create orchestrator with large goals to trigger token limit
        let largeGoals = Array(repeating: "Explore the entire workspace in detail", count: 100)

        let orchestrator = ExploreOrchestrator(
            workspaceRoot: "/tmp/test",
            goals: largeGoals,
            policy: ExplorationPolicy(),
            streamHandler: { _ in }
        )

        // Should not crash when hitting token limit
        _ = try await orchestrator.startExploration()

        // TODO: Assert transcript was pruned
    }

    func testNativeToolCalling() async throws {
        // Enable native tool calling
        let policy = ExplorationPolicy(use_native_tool_calling: true)

        let orchestrator = ExploreOrchestrator(
            workspaceRoot: "/tmp/test",
            goals: ["List sessions"],
            policy: policy,
            streamHandler: { _ in }
        )

        let summary = try await orchestrator.startExploration()

        // Should have results from tool calls
        XCTAssertFalse(summary.followups.isEmpty)
    }

    func testFallbackToTextPlan() async throws {
        // Disable native tool calling
        let policy = ExplorationPolicy(use_native_tool_calling: false)

        let orchestrator = ExploreOrchestrator(
            workspaceRoot: "/tmp/test",
            goals: ["List sessions"],
            policy: policy,
            streamHandler: { _ in }
        )

        let summary = try await orchestrator.startExploration()

        // Should still work via text plan parsing
        XCTAssertFalse(summary.followups.isEmpty)
    }
}
#endif
```

**Acceptance criteria**:
- [ ] All tests pass on iOS 26+ / macOS 26+ with FM available
- [ ] Tests verify persistent session behavior
- [ ] Tests verify token budget enforcement
- [ ] Tests verify native tool calling works
- [ ] Tests verify fallback path still works

---

## Rollout Checklist

1. [ ] Implement persistent session (Task 1)
2. [ ] Implement native tool calling loop (Task 2)
3. [ ] Add token budget management (Task 3)
4. [ ] Add streaming support (Task 4)
5. [ ] Update FM tools to structured outputs (Task 5)
6. [ ] Add feature flag (Task 6)
7. [ ] Write tests (Task 7)
8. [ ] Test on real device with FM available
9. [ ] Enable `use_native_tool_calling` for internal testing
10. [ ] Monitor ACP streams for correctness
11. [ ] Validate multi-turn reasoning quality
12. [ ] Flip default to native tool calling
13. [ ] Remove legacy text plan parsing code
14. [ ] Update documentation

## Notes for Coding Agents

- **Foundation Models API**: The actual API for detecting tool calls and returning results may differ from the sketches above. Consult Apple's documentation for the correct approach.
- **Transcript API**: The Transcript type and its methods may vary by SDK version. Adjust accordingly.
- **Error Handling**: Add proper error handling for all async calls. Don't swallow errors.
- **Logging**: Keep detailed logs for debugging. Use `[Orchestrator]` prefix consistently.
- **ACP Compliance**: All streamed updates must follow ACP schema. Don't invent new message types.

## Success Metrics

After implementation, verify:

1. **No manual parsing**: Zero regex-based plan parsing (remove lines 277-1016 from ExploreOrchestrator.swift)
2. **Multi-turn works**: Session persists across multiple `startExploration()` calls
3. **Token budget safe**: No crashes when approaching 4096 token limit
4. **Streaming works**: iOS receives partial updates during generation
5. **Tools invoked correctly**: Model calls tools directly, no parsing failures
6. **Structured outputs**: Tool results are type-safe `@Generable` objects

## Questions?

See `tool-calling-audit.md` for rationale and high-level analysis.

For Foundation Models API details, consult:
- `docs/foundation-models/tool-calling.md`
- `docs/foundation-models/stateful-sessions.md`
- Apple's official Foundation Models documentation
