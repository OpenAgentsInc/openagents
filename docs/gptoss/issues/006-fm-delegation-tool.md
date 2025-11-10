# Issue #6: Add gptoss.generate Tool to Foundation Models

**Phase:** 3 (Routing)
**Priority:** P1
**Estimated Effort:** 1 day
**Dependencies:** #5 (Registration must be complete)
**Related Issues:** #7 (Task Routing Logic), #1469 (codex.run pattern)

---

## Summary

Add `gptoss.generate` tool to Foundation Models session in `OpenAgentsLocalProvider`, enabling automatic delegation of code generation and complex reasoning tasks to GPTOSS 20B. Ensure harmony‑compliant role construction and channel safety when delegating.

## Context

Following the existing `codex.run` tool pattern (FMTool_CodexRun), we add a new tool that allows Foundation Models to invoke GPTOSS when it determines a task requires heavyweight reasoning or code generation.

## Acceptance Criteria

- [ ] `FMTool_GPTOSSGenerate` struct added to OpenAgentsLocalProvider
- [ ] Tool registered in Foundation Models session
- [ ] Tool properly delegates to GPTOSS via local RPC
- [ ] Temperature/max_tokens parameters supported
- [ ] Workspace context passed through
- [ ] ACP tool_call emitted for UI visibility
- [ ] Harmony roles respected: GPTOSS provider constructs messages via ChatSession (no raw prompts)
- [ ] CoT/`analysis` channel not shown to users; only `final` channel streamed to UI
- [ ] Integration test passes
- [ ] Works alongside existing codex.run tool

## Technical Implementation

**File:** `ios/OpenAgentsCore/Sources/OpenAgentsCore/Agents/OpenAgentsLocalProvider.swift`

Add after existing FMTool_CodexRun:

```swift
#if canImport(FoundationModels)
@available(iOS 26.0, macOS 26.0, *)
extension OpenAgentsLocalProvider {
    struct FMTool_GPTOSSGenerate: Tool {
        let name = "gptoss.generate"
        let description = """
        Delegate a code generation, documentation, or complex reasoning task to GPTOSS 20B (local LLM). \
        Use for:
        - Multi-line code generation (functions, classes, modules)
        - Documentation (README, API docs, guides)
        - Complex analysis or explanations (>200 tokens)
        - Refactoring or architectural planning

        Do NOT use for quick answers, meta questions, or simple tasks (<140 tokens).
        """
        typealias Output = String

        private let sessionId: ACPSessionId
        private let updateHub: SessionUpdateHub
        private let workspaceRoot: String?
        private weak var server: DesktopWebSocketServer?

        init(sessionId: ACPSessionId, updateHub: SessionUpdateHub, workspaceRoot: String?, server: DesktopWebSocketServer?) {
            self.sessionId = sessionId
            self.updateHub = updateHub
            self.workspaceRoot = workspaceRoot
            self.server = server
        }

        @Generable
        struct Arguments {
            @Guide(description: "The user's prompt to pass to GPTOSS")
            var user_prompt: String

            @Guide(description: "Task type: 'code_generation', 'documentation', 'analysis', 'refactoring'")
            var task_type: String?

            @Guide(description: "Workspace root directory (absolute path)")
            var workspace_root: String?

            @Guide(description: "Temperature (0.0-1.0, default 0.7)")
            var temperature: Double?

            @Guide(description: "Maximum tokens to generate (default: unlimited)")
            var max_tokens: Int?
        }

        func call(arguments a: Arguments) async throws -> Output {
            guard let server = self.server else {
                throw GPTOSSError.serverUnavailable
            }

            // Check if GPTOSS is available
            let provider = await server.agentRegistry.provider(for: .gptoss_20b)
            guard let gptossProvider = provider else {
                return "gptoss.generate unavailable: Provider not registered. Falling back to inline response."
            }

            let available = await gptossProvider.isAvailable()
            guard available else {
                return "gptoss.generate unavailable: Insufficient memory or not macOS. Falling back to inline response."
            }

            let callId = UUID().uuidString

            // Build arguments for ACP tool_call
            var args: [String: AnyEncodable] = [:]
            args["provider"] = AnyEncodable("gptoss")
            args["user_prompt"] = AnyEncodable(a.user_prompt)
            if let t = a.task_type { args["task_type"] = AnyEncodable(t) }
            if let wr = a.workspace_root ?? workspaceRoot { args["workspace_root"] = AnyEncodable(wr) }
            if let temp = a.temperature { args["temperature"] = AnyEncodable(temp) }
            if let max = a.max_tokens { args["max_tokens"] = AnyEncodable(max) }

            // Emit ACP tool_call for UI visibility
            let call = ACPToolCallWire(call_id: callId, name: name, arguments: args)
            await updateHub.sendSessionUpdate(sessionId: sessionId, update: .toolCall(call))

            // Apply workspace root if supplied
            if let wr = a.workspace_root ?? workspaceRoot {
                server.workingDirectory = URL(fileURLWithPath: wr)
            }

            // Switch to GPTOSS mode and invoke
            await server.localSessionSetMode(sessionId: sessionId, mode: .gptoss_20b)

            let text = Self.composeDelegationPrompt(
                taskType: a.task_type,
                userPrompt: a.user_prompt,
                workspaceRoot: a.workspace_root ?? workspaceRoot,
                temperature: a.temperature,
                maxTokens: a.max_tokens
            )

            let req = ACP.Agent.SessionPromptRequest(
                session_id: sessionId,
                content: [.text(.init(text: text))]
            )
            try? await server.localSessionPrompt(request: req)

            return "gptoss.generate dispatched"
        }

        // Note: The provider should construct harmony‑compliant messages internally using ChatSession.
        // The text below is passed as the user prompt; provider prepends any developer/system messages as needed.
        private static func composeDelegationPrompt(
            taskType: String?,
            userPrompt: String,
            workspaceRoot: String?,
            temperature: Double?,
            maxTokens: Int?
        ) -> String {
            var parts: [String] = []
            if let t = taskType, !t.isEmpty { parts.append("Task type: \(t)") }
            if let wr = workspaceRoot { parts.append("Workspace: \(wr)") }
            if let temp = temperature { parts.append("Temperature: \(temp)") }
            if let max = maxTokens { parts.append("Max tokens: \(max)") }
            parts.append("")
            parts.append(userPrompt)
            return parts.joined(separator: "\n")
        }
    }
}
#endif
```

**Register Tool in ensureSession**:

```swift
private func ensureSession(...) async throws -> LanguageModelSession {
    // ... existing code ...

    var tools: [any Tool] = []
    tools.append(FMTool_CodexRun(sessionId: sessionId, updateHub: updateHub, workspaceRoot: workspaceRoot, server: server))
    tools.append(FMTool_GPTOSSGenerate(sessionId: sessionId, updateHub: updateHub, workspaceRoot: workspaceRoot, server: server))  // NEW

    let s = LanguageModelSession(model: model, tools: tools, instructions: instructions)
    // ... rest of code ...
}
```

## Testing

```swift
func testGPTOSSDelegation() async throws {
    #if canImport(FoundationModels)
    if #available(macOS 26.0, *) {
        let server = DesktopWebSocketServer()
        let localProvider = OpenAgentsLocalProvider()
        let gptossProvider = GPTOSSAgentProvider()

        await server.agentRegistry.register(localProvider)
        await server.agentRegistry.register(gptossProvider)

        let sessionId = ACPSessionId(value: "test-gptoss-delegation")
        let updateHub = MockSessionUpdateHub()

        // Prompt that should trigger GPTOSS delegation
        let context = AgentContext(workingDirectory: nil, client: nil, server: server, metadata: nil)
        _ = try await localProvider.start(
            sessionId: sessionId,
            prompt: "Generate a comprehensive Swift actor implementation for managing WebSocket connections with reconnection logic and exponential backoff",
            context: context,
            updateHub: updateHub
        )

        // Verify FM invoked gptoss.generate tool
        let updates = await updateHub.getUpdates(for: sessionId)
        let hasGPTOSSTool = updates.contains { update in
            if case .toolCall(let call) = update {
                return call.name == "gptoss.generate"
            }
            return false
        }

        XCTAssertTrue(hasGPTOSSTool, "FM should invoke gptoss.generate for code generation tasks")
    }
    #endif
}
```

## References

- FMTool_CodexRun implementation (same file)
- Issue #1469 (Wire FM codex.run tool)
- Integration Spec Section 5.3

## Definition of Done

- [ ] FMTool_GPTOSSGenerate implemented
- [ ] Tool registered in FM session
- [ ] Integration test passes
- [ ] Delegation works end-to-end (FM → GPTOSS → UI)
- [ ] Tool call appears in UI
- [ ] Fallback works when GPTOSS unavailable
- [ ] Committed with message: "Add gptoss.generate tool for FM delegation"
