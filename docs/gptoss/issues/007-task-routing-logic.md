# Issue #7: Implement Intelligent Task Routing Logic

**Phase:** 3 (Routing)
**Priority:** P1
**Estimated Effort:** 1 day
**Dependencies:** #6 (gptoss.generate tool must exist)
**Related Issues:** #7 (Routing Logic)

---

## Summary

Update Foundation Models instructions and decision rubric to intelligently route tasks between lightweight inline responses, GPTOSS delegation, and external agents (Codex/Claude).

## Acceptance Criteria

- [ ] FM instructions updated with clear routing rubric
- [ ] FM correctly identifies code generation tasks → GPTOSS
- [ ] FM correctly identifies documentation tasks → GPTOSS
- [ ] FM still answers meta questions inline (no tool call)
- [ ] FM still generates short summaries inline
- [ ] Fallback chain works (GPTOSS → Codex → Claude Code)
- [ ] Routing metrics/logging added
- [ ] Integration tests cover key scenarios

## Technical Implementation

**Update FM Instructions in OpenAgentsLocalProvider**:

```swift
let instructions = Instructions("""
You are OpenAgents. Respond with 2-3 sentences for simple queries, or use tools for complex tasks.

- Identify as \"We are OpenAgents.\" Always use first-person plural ("We ___", not "I ___").

**Decision Rubric for Tool Selection:**

1. **Inline Response** (no tool) - Use for:
   - Meta questions: "who are you?", "what can you do?", "how do you work?"
   - Capability questions: "can you...", "do you support..."
   - Short explanations (<140 tokens)
   - Conversation titles, summaries, tags
   - Quick acknowledgments

2. **gptoss.generate** - Use for:
   - Code generation (functions, classes, modules, refactoring)
   - Documentation (README, API docs, tutorials, guides)
   - Complex explanations or analysis (>200 tokens)
   - Multi-step reasoning or planning
   - Architectural decisions
   - Long-form content

3. **codex.run** - Use for:
   - User explicitly mentions "Codex" or "codex"
   - GPTOSS unavailable (fallback)
   - Tasks requiring Codex-specific features

4. **Fallback Chain:**
   - Try gptoss.generate first for code/docs tasks
   - If GPTOSS unavailable, fall back to codex.run
   - If both unavailable, provide inline guidance

**Examples:**

Q: who are you?
A: We are OpenAgents, a native agent orchestration system. Ready to assist. [No tool]

Q: what can you do?
A: We coordinate agents and help with coding tasks. We can generate code, analyze repositories, and create documentation. [No tool]

Q: Generate a Swift function that validates email addresses.
A: [Calls gptoss.generate with user_prompt]

Q: Write comprehensive documentation for this API.
A: [Calls gptoss.generate with task_type: "documentation"]

Q: Use Codex to implement this feature.
A: [Calls codex.run as explicitly requested]

**Key Principle:** Mentioning an agent name is NOT alone sufficient to invoke a tool. Only invoke tools when execution is actually needed.
""")
```

**Add Routing Metrics**:

```swift
public actor RoutingMetrics {
    private var decisions: [RoutingDecision] = []

    public struct RoutingDecision: Codable {
        var timestamp: Date
        var prompt: String
        var decision: Decision
        var reason: String?

        enum Decision: String, Codable {
            case inline
            case gptoss
            case codex
            case claude
        }
    }

    public func record(prompt: String, decision: RoutingDecision.Decision, reason: String?) {
        decisions.append(RoutingDecision(
            timestamp: Date(),
            prompt: prompt,
            decision: decision,
            reason: reason
        ))
    }

    public func getMetrics() -> [RoutingDecision] {
        decisions
    }
}
```

## Testing

**Golden Test Cases**:

```swift
struct RoutingTestCase {
    var prompt: String
    var expectedDecision: Decision
    var reason: String
}

let goldenCases: [RoutingTestCase] = [
    // Inline responses
    RoutingTestCase(
        prompt: "who are you?",
        expectedDecision: .inline,
        reason: "Meta question about identity"
    ),
    RoutingTestCase(
        prompt: "can you help me with coding?",
        expectedDecision: .inline,
        reason: "Capability question"
    ),

    // GPTOSS delegation
    RoutingTestCase(
        prompt: "Generate a Swift actor for managing database connections",
        expectedDecision: .gptoss,
        reason: "Code generation task"
    ),
    RoutingTestCase(
        prompt: "Write a comprehensive README for this project with architecture diagrams",
        expectedDecision: .gptoss,
        reason: "Documentation task"
    ),
    RoutingTestCase(
        prompt: "Analyze this codebase and suggest performance optimizations",
        expectedDecision: .gptoss,
        reason: "Complex analysis task"
    ),

    // Explicit requests
    RoutingTestCase(
        prompt: "Use Codex to implement this feature",
        expectedDecision: .codex,
        reason: "Explicit Codex request"
    ),
]

func testRoutingDecisions() async throws {
    let provider = OpenAgentsLocalProvider()
    let server = DesktopWebSocketServer()
    await server.agentRegistry.register(GPTOSSAgentProvider())
    await server.agentRegistry.register(CodexAgentProvider())

    for testCase in goldenCases {
        let updateHub = MockSessionUpdateHub()
        let sessionId = ACPSessionId(value: UUID().uuidString)

        _ = try await provider.start(
            sessionId: sessionId,
            prompt: testCase.prompt,
            context: AgentContext(workingDirectory: nil, client: nil, server: server, metadata: nil),
            updateHub: updateHub
        )

        let updates = await updateHub.getUpdates(for: sessionId)
        let actualDecision = inferDecision(from: updates)

        XCTAssertEqual(
            actualDecision,
            testCase.expectedDecision,
            "For prompt '\(testCase.prompt)': Expected \(testCase.expectedDecision), got \(actualDecision). Reason: \(testCase.reason)"
        )
    }
}

func inferDecision(from updates: [ACP.SessionUpdate]) -> Decision {
    for update in updates {
        if case .toolCall(let call) = update {
            if call.name == "gptoss.generate" { return .gptoss }
            if call.name == "codex.run" { return .codex }
        }
    }
    return .inline
}
```

## References

- OpenAgentsLocalProvider current instructions
- FMTool_CodexRun decision rubric
- Integration Spec Section 4

## Definition of Done

- [ ] FM instructions updated with routing rubric
- [ ] Golden test cases pass (>90% accuracy)
- [ ] Routing metrics logged for analysis
- [ ] Fallback chain works
- [ ] No false positives (meta questions don't trigger tools)
- [ ] Committed with message: "Implement intelligent task routing for GPTOSS"
