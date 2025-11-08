# Issue #008: iOS Bridge Integration

**Component**: Bridge Layer - iOS Monitoring UI
**Priority**: P2 (Medium)
**Estimated Effort**: 3-4 days
**Dependencies**: #001-#007 (all orchestration components)
**Assignee**: TBD

---

## Overview

Add iOS monitoring UI for overnight agents: real-time task queue, session status, FM decision rationale, PR preview with approve/cancel.

**Location**: `ios/OpenAgents/Views/OvernightMonitoringView.swift`

---

## Requirements

1. **TaskQueueCard**: Pending/active/done counts, tap to see details
2. **SessionCard**: Agent type, progress, tool call count, elapsed time
3. **DecisionCard**: Task description, rationale, confidence
4. **PRPreviewCard**: PR title/body, approve/edit/cancel buttons
5. **Bridge Integration**: New JSON-RPC methods for orchestration state

---

## Bridge Messages

```json
// macOS → iOS: Task queued
{
  "jsonrpc": "2.0",
  "method": "orchestration/task_queued",
  "params": {
    "task_id": "uuid",
    "decision": {
      "task": "Refactor BridgeManager...",
      "agent": "claude-code",
      "priority": "high",
      "rationale": "..."
    }
  }
}

// macOS → iOS: Task started
{
  "jsonrpc": "2.0",
  "method": "orchestration/task_started",
  "params": {
    "task_id": "uuid",
    "session_id": "uuid",
    "agent": "claude-code"
  }
}

// macOS → iOS: Task completed
{
  "jsonrpc": "2.0",
  "method": "orchestration/task_completed",
  "params": {
    "task_id": "uuid",
    "pr_number": 42,
    "pr_url": "https://github.com/..."
  }
}

// iOS → macOS: Approve PR
{
  "jsonrpc": "2.0",
  "method": "orchestration/approve_pr",
  "params": {
    "task_id": "uuid",
    "approved": true
  },
  "id": 123
}
```

---

## SwiftUI Views

```swift
struct OvernightMonitoringView: View {
    @EnvironmentObject var bridgeManager: BridgeManager
    @State private var queueState: QueueState = .empty

    var body: some View {
        VStack(spacing: 20) {
            TaskQueueCard(state: queueState)

            if !queueState.activeTasks.isEmpty {
                ForEach(queueState.activeTasks) { task in
                    SessionCard(task: task)
                }
            }

            if !queueState.decisions.isEmpty {
                ForEach(queueState.decisions) { decision in
                    DecisionCard(decision: decision)
                }
            }

            if let pr = queueState.pendingPR {
                PRPreviewCard(pr: pr, onApprove: { approvePR(pr) })
            }
        }
        .padding()
        .onAppear {
            subscribeToBridgeMessages()
        }
    }

    private func subscribeToBridgeMessages() {
        // Listen for orchestration/* methods
    }

    private func approvePR(_ pr: PendingPR) {
        Task {
            try await bridgeManager.sendRequest(
                method: "orchestration/approve_pr",
                params: ["task_id": pr.taskId, "approved": true]
            )
        }
    }
}

struct TaskQueueCard: View {
    let state: QueueState

    var body: some View {
        HStack(spacing: 40) {
            VStack {
                Text("\(state.pending)")
                    .font(.system(size: 32, weight: .bold))
                Text("Pending")
                    .font(.caption)
                    .foregroundColor(.secondary)
            }

            VStack {
                Text("\(state.active)")
                    .font(.system(size: 32, weight: .bold))
                    .foregroundColor(.blue)
                Text("Active")
                    .font(.caption)
                    .foregroundColor(.secondary)
            }

            VStack {
                Text("\(state.completed)")
                    .font(.system(size: 32, weight: .bold))
                    .foregroundColor(.green)
                Text("Done")
                    .font(.caption)
                    .foregroundColor(.secondary)
            }
        }
        .padding()
        .background(Material.ultraThin)
        .cornerRadius(12)
    }
}

struct SessionCard: View {
    let task: ActiveTask

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack {
                Text(task.agent.rawValue)
                    .font(.headline)
                Spacer()
                Text(formatElapsed(task.elapsedTime))
                    .font(.caption)
                    .foregroundColor(.secondary)
            }

            Text(task.decision.task)
                .font(.body)

            ProgressView(value: Double(task.toolCallCount) / 50.0)
                .progressViewStyle(.linear)

            Text("\(task.toolCallCount) tool calls")
                .font(.caption)
                .foregroundColor(.secondary)
        }
        .padding()
        .background(Material.regular)
        .cornerRadius(12)
    }
}
```

---

## macOS Bridge Handler

```swift
// In DesktopWebSocketServer
func handleOrchestratorUpdates() {
    Task {
        for await update in TaskQueue.shared.updates {
            switch update {
            case .enqueued(let taskId):
                let task = try await TaskQueue.shared.find(taskId)
                try await broadcast(
                    method: "orchestration/task_queued",
                    params: ["task_id": taskId, "decision": task.decision.toDictionary()]
                )

            case .statusChanged(let taskId, let status):
                if status == .in_progress {
                    let task = try await TaskQueue.shared.find(taskId)
                    try await broadcast(
                        method: "orchestration/task_started",
                        params: ["task_id": taskId, "session_id": task.sessionId!, "agent": task.decision.agent.rawValue]
                    )
                } else if status == .completed {
                    // Send completion with PR info
                }

            default:
                break
            }
        }
    }
}
```

---

## Testing

1. `testBridgeMessagesFlow()` - macOS sends, iOS receives
2. `testPRApproval()` - iOS sends approval, macOS acts
3. `testRealTimeUpdates()` - UI updates as tasks progress

---

## Acceptance Criteria

- [ ] All bridge messages implemented
- [ ] iOS UI updates in real-time
- [ ] PR approval flow works
- [ ] No UI lag or crashes
- [ ] Tests pass

---

## References

- docs/ios-bridge/README.md
