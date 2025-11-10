# Issue #5: Register GPTOSS in Agent Registry

**Phase:** 2 (Integration)
**Priority:** P0 (Blocking)
**Estimated Effort:** 2-4 hours
**Dependencies:** #2 (Provider Core), #4 (Streaming)
**Related Issues:** #6 (FM Delegation)

---

## Summary

Register `GPTOSSAgentProvider` in the agent registry so it can be invoked via `session.set_mode` and appears in the agent selector UI.

## Acceptance Criteria

- [ ] GPTOSS provider registered in `DesktopWebSocketServer.registerAgentProviders()`
- [ ] Can invoke via JSON-RPC: `session.set_mode({ "mode": "gptoss_20b" })`
- [ ] GPTOSS appears in agent selector (macOS UI)
- [ ] Availability check works (macOS + memory requirements)
- [ ] Session starts, generates, and completes successfully
- [ ] Switching between agents works (default → gptoss → codex)
- [ ] Integration tests pass

## Technical Implementation

**Update DesktopWebSocketServer.swift**:

```swift
private func registerAgentProviders() async {
    await agentRegistry.register(OpenAgentsLocalProvider())
    await agentRegistry.register(CodexAgentProvider())
    await agentRegistry.register(ClaudeCodeAgentProvider())
    await agentRegistry.register(GPTOSSAgentProvider())  // NEW
}
```

**Update Agent Selector UI** (macOS):

```swift
// In ChatMacOSView or agent selector
enum AgentOption: String, CaseIterable {
    case openagents = "OpenAgents"
    case gptoss = "GPTOSS 20B"
    case codex = "Codex"
    case claude = "Claude Code"

    var modeId: ACPSessionModeId {
        switch self {
        case .openagents: return .default_mode
        case .gptoss: return .gptoss_20b
        case .codex: return .codex
        case .claude: return .claude_code
        }
    }

    var badge: String? {
        switch self {
        case .gptoss: return "LOCAL"
        default: return nil
        }
    }
}
```

## Testing

**Integration Test**:

```swift
func testGPTOSSRegistrationAndInvocation() async throws {
    let server = DesktopWebSocketServer()
    // registerAgentProviders() called in init

    let sessionId = ACPSessionId(value: "test-gptoss")
    let updateHub = MockSessionUpdateHub()

    // Set mode to GPTOSS
    await server.localSessionSetMode(sessionId: sessionId, mode: .gptoss_20b)

    // Verify provider is registered and available
    let provider = await server.agentRegistry.provider(for: .gptoss_20b)
    XCTAssertNotNil(provider)

    let available = await provider?.isAvailable()
    XCTAssertTrue(available ?? false, "GPTOSS should be available on test Mac")

    // Start session
    let req = ACP.Agent.SessionPromptRequest(
        session_id: sessionId,
        content: [.text(.init(text: "Test prompt"))]
    )
    try await server.localSessionPrompt(request: req)

    // Verify updates received
    let updates = await updateHub.getUpdates(for: sessionId)
    XCTAssertGreaterThan(updates.count, 0)
}
```

**JSON-RPC Test** (via WebSocket):

```json
{
  "jsonrpc": "2.0",
  "id": "1",
  "method": "session.new",
  "params": {
    "mode": "gptoss_20b"
  }
}

// Response
{
  "jsonrpc": "2.0",
  "id": "1",
  "result": {
    "session_id": "sess_abc123"
  }
}

// Prompt
{
  "jsonrpc": "2.0",
  "id": "2",
  "method": "session.prompt",
  "params": {
    "session_id": "sess_abc123",
    "content": [{"type": "text", "text": "Generate a Swift function"}]
  }
}

// Updates stream via session/update notifications
```

## References

- DesktopWebSocketServer agent registration
- AgentRegistry implementation
- Integration Spec Section 5.2

## Definition of Done

- [ ] Provider registered in DesktopWebSocketServer
- [ ] Available via JSON-RPC session.set_mode
- [ ] Appears in UI agent selector
- [ ] Integration tests pass
- [ ] Manual testing successful
- [ ] Committed with message: "Register GPTOSS provider in agent registry"
