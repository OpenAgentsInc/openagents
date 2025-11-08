# Issue #004: Implement AgentCoordinator

**Component**: Agent Execution Layer
**Priority**: P0 (Critical Path)
**Estimated Effort**: 3-4 days
**Dependencies**: #003 (TaskQueue)
**Assignee**: TBD

---

## Overview

Delegate tasks to AgentProvider instances (Claude Code, Codex), monitor progress via ACP SessionUpdate stream, enforce time budgets, handle concurrent sessions.

**Key Change from Audit**: Use AgentRegistry instance (not .shared). Build proper AgentContext with SessionUpdateHub. Use actual provider.start() signature.

**Location**: `ios/OpenAgentsCore/Sources/OpenAgentsCore/Orchestration/AgentCoordinator.swift`

**References**:
- `AgentProvider.start()`: ios/OpenAgentsCore/Sources/OpenAgentsCore/Agents/AgentProvider.swift:23
- AgentRegistry instance: ios/OpenAgentsCore/Sources/OpenAgentsCore/DesktopBridge/DesktopWebSocketServer.swift:69

---

## Requirements

1. Delegate OrchestrationTask to appropriate AgentProvider
2. Subscribe to SessionUpdateHub for progress monitoring
3. Enforce time budget (cancel if exceeded)
4. Handle concurrent sessions (max 2 parallel)
5. Resume on recoverable errors
6. Collect results for PR generation

---

## Implementation

```swift
actor AgentCoordinator {
    private var activeSessions: [String: AgentSessionInfo] = [:]
    private let maxConcurrentSessions = 2
    private let agentRegistry: AgentRegistry
    private let sessionUpdateHub: SessionUpdateHub

    init(agentRegistry: AgentRegistry, sessionUpdateHub: SessionUpdateHub) {
        self.agentRegistry = agentRegistry
        self.sessionUpdateHub = sessionUpdateHub
    }

    func delegate(_ task: OrchestrationTask) async throws -> AgentSessionResult {
        // Get provider from registry (instance, not .shared)
        guard let provider = await agentRegistry.provider(for: task.decision.agent) else {
            throw CoordinatorError.agentNotAvailable(task.decision.agent)
        }

        // Build agent context
        let sessionId = UUID().uuidString
        let context = AgentContext(
            workingDirectory: FileManager.default.currentDirectoryURL,
            mcpServers: [],
            client: nil,
            metadata: ["orchestration_task_id": task.id]
        )

        // Start session with proper signature
        let handle = try await provider.start(
            sessionId: sessionId,
            prompt: task.decision.task,
            context: context,
            updateHub: sessionUpdateHub
        )

        // Monitor progress
        var toolCalls: [ACPToolCallWire] = []
        var lastUpdate = Date()

        for await update in SessionUpdateHub.shared.updates(for: sessionId) {
            lastUpdate = Date()

            switch update.type {
            case .tool_call(let call):
                toolCalls.append(call)
            case .agent_message_chunk:
                break  // Log progress
            case .error(let error):
                throw CoordinatorError.agentError(error)
            }

            // Check time budget
            if Date().timeIntervalSince(task.startedAt!) > task.decision.estimatedDuration * 1.5 {
                try await cancelSession(sessionId)
                throw CoordinatorError.timeBudgetExceeded
            }
        }

        return AgentSessionResult(
            sessionId: sessionId,
            agent: task.decision.agent,
            startedAt: task.startedAt!,
            completedAt: Date(),
            success: true,
            toolCalls: toolCalls,
            totalDuration: Date().timeIntervalSince(task.startedAt!),
            error: nil
        )
    }

    func monitorSession(_ sessionId: String) -> AsyncStream<ACPSessionUpdate> {
        SessionUpdateHub.shared.updates(for: sessionId)
    }

    func cancelSession(_ sessionId: String) async throws {
        guard let info = activeSessions[sessionId] else { return }
        let provider = await AgentRegistry.shared.provider(for: info.agent)
        try await provider?.cancel(sessionId: sessionId)
        activeSessions.removeValue(forKey: sessionId)
    }

    var activeSessions: [String: AgentSessionInfo] { get async }
}

struct AgentSessionResult {
    let sessionId: String
    let agent: AgentType
    let startedAt: Date
    let completedAt: Date
    let success: Bool
    let toolCalls: [ACPToolCallWire]
    let totalDuration: TimeInterval
    let error: String?
}
```

---

## Testing

1. `testDelegateTask_MockAgent()` - Basic delegation
2. `testTimeBudgetEnforcement()` - Cancel after timeout
3. `testConcurrentSessions()` - Claude + Codex in parallel
4. `testErrorRecovery()` - Resume on recoverable error

---

## Acceptance Criteria

- [ ] Delegates to correct AgentProvider
- [ ] Monitors ACP stream correctly
- [ ] Enforces time budget
- [ ] Handles max concurrent sessions
- [ ] All tests pass (≥85% coverage)

---

## References

- Architecture: `architecture.md` - AgentCoordinator section
- Existing: AgentRegistry, AgentProvider, SessionUpdateHub
