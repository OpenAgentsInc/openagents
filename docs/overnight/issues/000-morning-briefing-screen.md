# Issue 000: Morning Briefing Screen Mockup

**Status**: 🔄 Reopened (integration updates in progress)
**Priority**: High (Next Implementation)
**Platform**: iOS + macOS
**Dependencies**: ACP SessionUpdate types, OrchestrationTask (issue #003), TaskDecision (issue #002), PRAutomationService (issue #005), SessionAnalyzeResult
**Completed**: Nov 8, 2025

---

## Overview

When a user returns to their computer in the morning after overnight orchestration has run, they should see a comprehensive, actionable summary of all autonomous work completed. This screen is the "morning ritual" interface that transforms hours of autonomous agent work into a quick, scannable overview with drill-down capabilities.

**Key Goals**:
- Glanceable summary in < 5 seconds
- Clear action items (PRs to review/merge)
- Full transparency into FM decisions and agent execution
- Drill-down to any level of detail desired
- Celebrate wins, surface issues clearly

---

## User Story

> "I wake up, grab coffee, open my Mac. I see that overnight my agents created 6 pull requests. The summary shows Claude Code refactored error handling in BridgeManager (something I'd mentioned wanting to do), and Codex added comprehensive tests for the WebSocket server. I can see the FM orchestrator chose these tasks based on my recent session history. I tap on the first PR, see the diff looks good, approve and merge it—all before my first sip of coffee. I mark one failed task to retry tonight. Total time: 2 minutes. Value delivered: 3+ hours of quality engineering work."

---

## UI Layout Mockup

### macOS: Full Morning Dashboard

```
┌─────────────────────────────────────────────────────────────────────┐
│ Morning Briefing                                   [Export] [Close] │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  🌙 Orchestration Run Complete                                     │
│  Friday, Nov 8, 2025 • 1:00 AM - 5:00 AM (4 hours)                │
│                                                                     │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐            │
│  │ 8 Decisions  │  │ 6 Completed  │  │ 6 PRs Created│            │
│  │ Made         │  │ Tasks        │  │              │            │
│  └──────────────┘  └──────────────┘  └──────────────┘            │
│                                                                     │
├─────────────────────────────────────────────────────────────────────┤
│ Agent Performance                                                   │
│                                                                     │
│ Claude Code  ████████████████░░░░  67% (4 tasks)   Avg: 28 min    │
│ Codex        ████████░░░░░░░░░░░░  33% (2 tasks)   Avg: 18 min    │
│                                                                     │
│ Files Changed: 12 files • +847 / -423 lines                        │
│ Tool Calls: 87 total (47 edit_file, 25 read_file, 15 run_bash)    │
├─────────────────────────────────────────────────────────────────────┤
│ Pull Requests                                      [View on GitHub] │
│                                                                     │
│ ✓ #42  Refactor BridgeManager error handling                      │
│        Merged 5 min ago • Claude Code • 28m duration               │
│        +145 / -67 lines in 3 files                [View Details →] │
│                                                                     │
│ ⏳ #43  Add comprehensive tests for DesktopWebSocketServer         │
│        Awaiting review • Codex • 18m duration                      │
│        +423 / -12 lines in 5 files                 [Review PR →]   │
│                                                                     │
│ ⏳ #44  Refactor SessionUpdateHub concurrency                      │
│        Awaiting review • Claude Code • 32m duration                │
│        +98 / -145 lines in 2 files                 [Review PR →]   │
│                                                                     │
│ ⏳ #45  Add Foundation Models caching layer                        │
│        Awaiting review • Claude Code • 25m duration                │
│        +234 / -45 lines in 4 files                 [Review PR →]   │
│                                                                     │
│ ⏳ #46  Generate API documentation from code                       │
│        Awaiting review • Codex • 15m duration                      │
│        +567 / -23 lines in 8 files                 [Review PR →]   │
│                                                                     │
│ ⏳ #47  Optimize TaskQueue database queries                        │
│        Awaiting review • Claude Code • 22m duration                │
│        +89 / -156 lines in 2 files                 [Review PR →]   │
│                                                                     │
├─────────────────────────────────────────────────────────────────────┤
│ Orchestration Decisions                            [Show All (8) ↓]│
│                                                                     │
│ 💡 2:30 AM • High Priority • Confidence: 87%                       │
│    Task: Refactor BridgeManager error handling with Result types  │
│    Agent: Claude Code                                              │
│    Status: ✓ Completed → PR #42                                   │
│                                                                     │
│    Rationale: "BridgeManager.swift touched 25 times in recent     │
│    sessions with user frequently requesting error handling        │
│    improvements. Current implementation uses optional returns      │
│    which masks failure modes. Swift Result types provide better   │
│    error propagation and type safety..."                          │
│                                                                     │
│    [View Full Decision Analysis →]                                 │
│                                                                     │
│ 💡 2:00 AM • Medium Priority • Confidence: 92%                     │
│    Task: Generate comprehensive tests for DesktopWebSocketServer  │
│    Agent: Codex                                                    │
│    Status: ✓ Completed → PR #43                                   │
│                                                                     │
│    Rationale: "DesktopWebSocketServer modified 18 times with low  │
│    test coverage (current: 45%). Recent sessions show connection  │
│    reliability concerns. Comprehensive tests needed for critical  │
│    bridge infrastructure..."                                       │
│                                                                     │
│    [View Full Decision Analysis →]                                 │
│                                                                     │
│ 💡 1:45 AM • High Priority • Confidence: 78%                       │
│    Task: Refactor SessionUpdateHub concurrency patterns           │
│    Agent: Claude Code                                              │
│    Status: ✓ Completed → PR #44                                   │
│    [View Full Decision Analysis →]                                 │
│                                                                     │
├─────────────────────────────────────────────────────────────────────┤
│ Issues & Alerts                                                     │
│                                                                     │
│ ⚠️  1 Task Failed                                                  │
│     Task: "Add SwiftUI previews to all views"                     │
│     Agent: Codex                                                   │
│     Error: Build failed - missing ColorScheme import              │
│     [Retry Tonight] [View Session Logs →]                         │
│                                                                     │
│ ℹ️  1 Task Skipped                                                 │
│     Task: "Optimize image asset compression"                      │
│     Reason: Time budget exceeded (4hr limit reached)              │
│     [Reschedule] [Adjust Priority]                                │
│                                                                     │
├─────────────────────────────────────────────────────────────────────┤
│ Most Modified Files                                                 │
│                                                                     │
│ BridgeManager.swift                    3 PRs, 245 lines changed   │
│ DesktopWebSocketServer.swift           2 PRs, 189 lines changed   │
│ SessionUpdateHub.swift                 2 PRs, 167 lines changed   │
│ TaskQueue.swift                        1 PR, 98 lines changed     │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

### iOS: Mobile Morning Briefing

```
┌──────────────────────────────────┐
│ < Back          Morning Briefing │
├──────────────────────────────────┤
│                                  │
│ 🌙 Orchestration Run Complete   │
│ Nov 8, 2025 • 1-5 AM (4 hrs)    │
│                                  │
│ ┌──────────────────────────────┐ │
│ │ 8 Decisions • 6 Completed    │ │
│ │ 6 PRs Created                │ │
│ └──────────────────────────────┘ │
│                                  │
│ Agent Performance                │
│ ┌──────────────────────────────┐ │
│ │ Claude Code         67%      │ │
│ │ ███████████████░░░           │ │
│ │ 4 tasks • Avg 28 min         │ │
│ │                              │ │
│ │ Codex               33%      │ │
│ │ ████████░░░░░░░░             │ │
│ │ 2 tasks • Avg 18 min         │ │
│ └──────────────────────────────┘ │
│                                  │
│ Pull Requests (6)        [View >]│
│ ┌──────────────────────────────┐ │
│ │ ✓ #42 Refactor BridgeManager │ │
│ │   Merged 5m ago              │ │
│ │   Claude Code • 28m          │ │
│ └──────────────────────────────┘ │
│                                  │
│ ┌──────────────────────────────┐ │
│ │ ⏳ #43 Add WebSocket tests   │ │
│ │   Awaiting review            │ │
│ │   Codex • 18m       [Review] │ │
│ └──────────────────────────────┘ │
│                                  │
│ ┌──────────────────────────────┐ │
│ │ ⏳ #44 Refactor concurrency  │ │
│ │   Awaiting review            │ │
│ │   Claude Code • 32m [Review] │ │
│ └──────────────────────────────┘ │
│                                  │
│ [Show All 6 PRs]                 │
│                                  │
│ Decisions (8)            [View >]│
│ ┌──────────────────────────────┐ │
│ │ 💡 2:30 AM • 87% confidence  │ │
│ │ Refactor error handling      │ │
│ │ Claude Code          [View >]│ │
│ └──────────────────────────────┘ │
│                                  │
│ ┌──────────────────────────────┐ │
│ │ 💡 2:00 AM • 92% confidence  │ │
│ │ Generate WebSocket tests     │ │
│ │ Codex                [View >]│ │
│ └──────────────────────────────┘ │
│                                  │
│ [Show All 8 Decisions]           │
│                                  │
│ Issues & Alerts                  │
│ ┌──────────────────────────────┐ │
│ │ ⚠️  1 Task Failed            │ │
│ │ Add SwiftUI previews         │ │
│ │ [Retry] [View Logs]          │ │
│ └──────────────────────────────┘ │
│                                  │
└──────────────────────────────────┘
```

---

## Mock Data Structures (ACP-Shaped)

### 1. Orchestration Run Summary

```swift
struct OrchestrationRunSummary: Codable {
    let runId: String  // UUID
    let startTime: Date  // 2025-11-08T01:00:00Z
    let endTime: Date    // 2025-11-08T05:00:00Z
    let duration: TimeInterval  // 14400 seconds (4 hours)

    // Orchestration metrics
    let totalCycles: Int  // 8 decision cycles
    let completedTasks: Int  // 6
    let failedTasks: Int  // 1
    let skippedTasks: Int  // 1
    let cancelledTasks: Int  // 0

    // PR metrics
    let prsCreated: Int  // 6
    let prsMerged: Int  // 1
    let prsAwaitingReview: Int  // 5
    let prsFailed: Int  // 0

    // Timing metrics
    let autonomousWorkTime: TimeInterval  // 11520 seconds (3.2 hours)
    let idleTime: TimeInterval  // 2880 seconds (48 minutes)

    // File metrics
    let filesChanged: Int  // 12
    let linesAdded: Int  // 847
    let linesRemoved: Int  // 423

    // Tool metrics
    let toolCallsTotal: Int  // 87
    let toolCallsByType: [String: Int]  // {"edit_file": 47, "read_file": 25, "run_bash": 15}

    // Agent breakdown
    let agentBreakdown: [ACPSessionModeId: AgentStats]
}

struct AgentStats: Codable {
    let tasksCompleted: Int
    let averageDuration: TimeInterval  // seconds
    let successRate: Double  // 0.0 - 1.0
    let toolCallsTotal: Int
}
```

**Mock Instance**:
```swift
let mockSummary = OrchestrationRunSummary(
    runId: "run_20251108_orchestration",
    startTime: Date(timeIntervalSince1970: 1731024000),  // Nov 8, 2025 1:00 AM
    endTime: Date(timeIntervalSince1970: 1731038400),    // Nov 8, 2025 5:00 AM
    duration: 14400,
    totalCycles: 8,
    completedTasks: 6,
    failedTasks: 1,
    skippedTasks: 1,
    cancelledTasks: 0,
    prsCreated: 6,
    prsMerged: 1,
    prsAwaitingReview: 5,
    prsFailed: 0,
    autonomousWorkTime: 11520,
    idleTime: 2880,
    filesChanged: 12,
    linesAdded: 847,
    linesRemoved: 423,
    toolCallsTotal: 87,
    toolCallsByType: [
        "edit_file": 47,
        "read_file": 25,
        "run_bash": 15
    ],
    agentBreakdown: [
        .claude_code: AgentStats(
            tasksCompleted: 4,
            averageDuration: 1680,  // 28 minutes
            successRate: 1.0,
            toolCallsTotal: 58
        ),
        .codex: AgentStats(
            tasksCompleted: 2,
            averageDuration: 1080,  // 18 minutes
            successRate: 0.67,  // 1 failed
            toolCallsTotal: 29
        )
    ]
)
```

### 2. Orchestration Tasks

```swift
struct OrchestrationTask: Codable, Identifiable {
    let id: String  // TaskID (UUID)
    let opHash: String  // Deduplication hash
    var status: TaskStatus
    let decision: TaskDecision
    var sessionId: String?  // Associated ACP session
    let createdAt: Date
    var startedAt: Date?
    var completedAt: Date?
    var error: String?
    let metadata: [String: String]
}

enum TaskStatus: String, Codable {
    case pending
    case in_progress
    case completed
    case failed
    case cancelled
    case skipped
}

struct TaskDecision: Codable {
    let task: String  // Human-readable task description
    let agent: ACPSessionModeId
    let priority: Priority
    let estimatedDuration: TimeInterval
    let rationale: String  // FM explanation
    let confidence: Double  // 0.0 - 1.0
    let context: DecisionContext?
}

enum Priority: String, Codable {
    case high
    case medium
    case low
}

struct DecisionContext: Codable {
    let fileFrequency: [String: Int]?
    let toolFrequency: [String: Int]?
    let userIntent: String?
    let recentGoals: [String]?
}
```

**Mock Tasks**:
```swift
let mockTasks = [
    OrchestrationTask(
        id: "task_001",
        opHash: "hash_bridge_refactor",
        status: .completed,
        decision: TaskDecision(
            task: "Refactor BridgeManager error handling with Swift Result types",
            agent: .claude_code,
            priority: .high,
            estimatedDuration: 1800,
            rationale: """
            BridgeManager.swift touched 25 times in recent sessions with user \
            frequently requesting error handling improvements. Current implementation \
            uses optional returns which masks failure modes. Swift Result types \
            provide better error propagation and type safety. High confidence based \
            on clear user intent and well-scoped task.
            """,
            confidence: 0.87,
            context: DecisionContext(
                fileFrequency: [
                    "BridgeManager.swift": 25,
                    "DesktopWebSocketServer.swift": 18,
                    "MobileWebSocketClient.swift": 15
                ],
                toolFrequency: [
                    "edit_file": 47,
                    "read_file": 25
                ],
                userIntent: "improve error handling and bridge reliability",
                recentGoals: ["refactor", "error handling", "type safety"]
            )
        ),
        sessionId: "session_abc123",
        createdAt: Date(timeIntervalSince1970: 1731027600),  // 2:00 AM
        startedAt: Date(timeIntervalSince1970: 1731027900),  // 2:05 AM
        completedAt: Date(timeIntervalSince1970: 1731029580), // 2:33 AM
        metadata: [
            "pr_number": "42",
            "pr_url": "https://github.com/OpenAgentsInc/openagents/pull/42",
            "pr_status": "merged",
            "files_changed": "3",
            "lines_added": "145",
            "lines_removed": "67",
            "branch": "agent/orchestration/bridge-error-handling"
        ]
    ),

    OrchestrationTask(
        id: "task_002",
        opHash: "hash_websocket_tests",
        status: .completed,
        decision: TaskDecision(
            task: "Generate comprehensive tests for DesktopWebSocketServer",
            agent: .codex,
            priority: .medium,
            estimatedDuration: 1200,
            rationale: """
            DesktopWebSocketServer modified 18 times with low test coverage \
            (current: 45%). Recent sessions show connection reliability concerns. \
            Comprehensive tests needed for critical bridge infrastructure. Codex \
            selected for its strong test generation capabilities.
            """,
            confidence: 0.92,
            context: DecisionContext(
                fileFrequency: [
                    "DesktopWebSocketServer.swift": 18,
                    "BridgeManager.swift": 25
                ],
                toolFrequency: [
                    "edit_file": 12,
                    "run_bash": 8
                ],
                userIntent: "improve test coverage for bridge",
                recentGoals: ["tests", "reliability", "coverage"]
            )
        ),
        sessionId: "session_def456",
        createdAt: Date(timeIntervalSince1970: 1731024000),  // 1:00 AM
        startedAt: Date(timeIntervalSince1970: 1731024300),  // 1:05 AM
        completedAt: Date(timeIntervalSince1970: 1731025380), // 1:23 AM
        metadata: [
            "pr_number": "43",
            "pr_url": "https://github.com/OpenAgentsInc/openagents/pull/43",
            "pr_status": "open",
            "files_changed": "5",
            "lines_added": "423",
            "lines_removed": "12",
            "branch": "agent/orchestration/websocket-tests"
        ]
    ),

    OrchestrationTask(
        id: "task_003",
        opHash: "hash_concurrency_refactor",
        status: .completed,
        decision: TaskDecision(
            task: "Refactor SessionUpdateHub concurrency patterns",
            agent: .claude_code,
            priority: .high,
            estimatedDuration: 2100,
            rationale: """
            SessionUpdateHub shows race condition patterns in recent crash logs. \
            File accessed 15 times with concurrency-related modifications. User has \
            mentioned wanting to adopt Swift 6 strict concurrency. High-value \
            refactor for app stability.
            """,
            confidence: 0.78,
            context: DecisionContext(
                fileFrequency: [
                    "SessionUpdateHub.swift": 15,
                    "AgentProvider.swift": 10
                ],
                toolFrequency: [
                    "edit_file": 18,
                    "read_file": 12
                ],
                userIntent: "adopt Swift 6 concurrency and fix race conditions",
                recentGoals: ["concurrency", "Swift 6", "stability"]
            )
        ),
        sessionId: "session_ghi789",
        createdAt: Date(timeIntervalSince1970: 1731022800),  // 12:40 AM
        startedAt: Date(timeIntervalSince1970: 1731023100),  // 12:45 AM
        completedAt: Date(timeIntervalSince1970: 1731025020), // 1:17 AM
        metadata: [
            "pr_number": "44",
            "pr_url": "https://github.com/OpenAgentsInc/openagents/pull/44",
            "pr_status": "open",
            "files_changed": "2",
            "lines_added": "98",
            "lines_removed": "145",
            "branch": "agent/orchestration/concurrency-refactor"
        ]
    ),

    OrchestrationTask(
        id: "task_007",
        opHash: "hash_swiftui_previews",
        status: .failed,
        decision: TaskDecision(
            task: "Add SwiftUI previews to all views",
            agent: .codex,
            priority: .low,
            estimatedDuration: 900,
            rationale: """
            Many SwiftUI views lack PreviewProvider implementations, slowing \
            development iteration. Low-hanging fruit for DX improvement. Codex \
            selected for its boilerplate generation strength.
            """,
            confidence: 0.65,
            context: nil
        ),
        sessionId: "session_xyz999",
        createdAt: Date(timeIntervalSince1970: 1731033600),  // 3:40 AM
        startedAt: Date(timeIntervalSince1970: 1731033900),  // 3:45 AM
        completedAt: Date(timeIntervalSince1970: 1731034500), // 3:55 AM
        error: "Build failed after adding previews. Error: 'Cannot find ColorScheme in scope'. Missing import SwiftUI in 3 files.",
        metadata: [
            "session_id": "session_xyz999",
            "error_type": "build_failure",
            "files_attempted": "8"
        ]
    ),

    OrchestrationTask(
        id: "task_008",
        opHash: "hash_image_optimization",
        status: .skipped,
        decision: TaskDecision(
            task: "Optimize image asset compression",
            agent: .codex,
            priority: .low,
            estimatedDuration: 600,
            rationale: """
            Asset catalog contains unoptimized images (avg 2.3MB per image). \
            Compression could reduce app size by ~15MB. Low priority but easy win.
            """,
            confidence: 0.55,
            context: nil
        ),
        sessionId: nil,
        createdAt: Date(timeIntervalSince1970: 1731038100),  // 4:55 AM
        startedAt: nil,
        completedAt: nil,
        metadata: [
            "skip_reason": "Time budget exceeded (4hr limit reached)",
            "reschedule": "next_run"
        ]
    )
]
```

### 3. Pull Request Data

```swift
struct PRSummary: Codable, Identifiable {
    let id: Int  // PR number
    let number: Int  // PR number
    let title: String
    let url: String
    let status: PRStatus
    let agent: ACPSessionModeId
    let createdAt: Date
    let mergedAt: Date?
    let filesChanged: Int
    let linesAdded: Int
    let linesRemoved: Int
    let branch: String
    let taskId: String  // Links back to OrchestrationTask
    let duration: TimeInterval  // Session duration
    let ciStatus: CIStatus?
}

enum PRStatus: String, Codable {
    case open
    case merged
    case closed
    case draft
}

enum CIStatus: String, Codable {
    case pending
    case success
    case failure
}
```

**Mock PRs**:
```swift
let mockPRs = [
    PRSummary(
        id: 42,
        number: 42,
        title: "Refactor BridgeManager error handling",
        url: "https://github.com/OpenAgentsInc/openagents/pull/42",
        status: .merged,
        agent: .claude_code,
        createdAt: Date(timeIntervalSince1970: 1731029580),
        mergedAt: Date(timeIntervalSince1970: 1731038100),
        filesChanged: 3,
        linesAdded: 145,
        linesRemoved: 67,
        branch: "agent/orchestration/bridge-error-handling",
        taskId: "task_001",
        duration: 1680,  // 28 minutes
        ciStatus: .success
    ),

    PRSummary(
        id: 43,
        number: 43,
        title: "Add comprehensive tests for DesktopWebSocketServer",
        url: "https://github.com/OpenAgentsInc/openagents/pull/43",
        status: .open,
        agent: .codex,
        createdAt: Date(timeIntervalSince1970: 1731025380),
        mergedAt: nil,
        filesChanged: 5,
        linesAdded: 423,
        linesRemoved: 12,
        branch: "agent/orchestration/websocket-tests",
        taskId: "task_002",
        duration: 1080,  // 18 minutes
        ciStatus: .success
    ),

    PRSummary(
        id: 44,
        number: 44,
        title: "Refactor SessionUpdateHub concurrency",
        url: "https://github.com/OpenAgentsInc/openagents/pull/44",
        status: .open,
        agent: .claude_code,
        createdAt: Date(timeIntervalSince1970: 1731025020),
        mergedAt: nil,
        filesChanged: 2,
        linesAdded: 98,
        linesRemoved: 145,
        branch: "agent/orchestration/concurrency-refactor",
        taskId: "task_003",
        duration: 1920,  // 32 minutes
        ciStatus: .pending
    ),

    PRSummary(
        id: 45,
        number: 45,
        title: "Add Foundation Models caching layer",
        url: "https://github.com/OpenAgentsInc/openagents/pull/45",
        status: .open,
        agent: .claude_code,
        createdAt: Date(timeIntervalSince1970: 1731028200),
        mergedAt: nil,
        filesChanged: 4,
        linesAdded: 234,
        linesRemoved: 45,
        branch: "agent/orchestration/fm-caching",
        taskId: "task_004",
        duration: 1500,  // 25 minutes
        ciStatus: .success
    ),

    PRSummary(
        id: 46,
        number: 46,
        title: "Generate API documentation from code",
        url: "https://github.com/OpenAgentsInc/openagents/pull/46",
        status: .open,
        agent: .codex,
        createdAt: Date(timeIntervalSince1970: 1731031800),
        mergedAt: nil,
        filesChanged: 8,
        linesAdded: 567,
        linesRemoved: 23,
        branch: "agent/orchestration/api-docs",
        taskId: "task_005",
        duration: 900,  // 15 minutes
        ciStatus: .success
    ),

    PRSummary(
        id: 47,
        number: 47,
        title: "Optimize TaskQueue database queries",
        url: "https://github.com/OpenAgentsInc/openagents/pull/47",
        status: .open,
        agent: .claude_code,
        createdAt: Date(timeIntervalSince1970: 1731035400),
        mergedAt: nil,
        filesChanged: 2,
        linesAdded: 89,
        linesRemoved: 156,
        branch: "agent/orchestration/taskqueue-optimization",
        taskId: "task_006",
        duration: 1320,  // 22 minutes
        ciStatus: .success
    )
]
```

### 4. ACP Session Update Samples

**For drill-down into session details**:

```swift
// Sample tool call from session_abc123 (BridgeManager refactor)
let sampleToolCalls = [
    ACPToolCallWire(
        call_id: "call_001",
        name: "read_file",
        arguments: [
            "file_path": AnyEncodable("ios/OpenAgentsCore/Sources/OpenAgentsCore/DesktopBridge/BridgeManager.swift")
        ]
    ),

    ACPToolCallWire(
        call_id: "call_002",
        name: "edit_file",
        arguments: [
            "file_path": AnyEncodable("ios/OpenAgentsCore/Sources/OpenAgentsCore/DesktopBridge/BridgeManager.swift"),
            "old_string": AnyEncodable("""
                func sendMessage(_ message: BridgeMessage) -> String? {
                    // ... returns nil on error
                }
                """),
            "new_string": AnyEncodable("""
                func sendMessage(_ message: BridgeMessage) -> Result<String, BridgeError> {
                    // ... returns Result type
                }
                """)
        ]
    ),

    ACPToolCallWire(
        call_id: "call_003",
        name: "run_bash",
        arguments: [
            "command": AnyEncodable("cd ios && xcodebuild test -workspace OpenAgents.xcworkspace -scheme OpenAgents -sdk iphonesimulator")
        ]
    )
]

let sampleToolCallUpdates = [
    ACPToolCallUpdateWire(
        call_id: "call_001",
        status: .completed,
        output: AnyEncodable("Read 456 lines from BridgeManager.swift"),
        error: nil
    ),

    ACPToolCallUpdateWire(
        call_id: "call_002",
        status: .completed,
        output: AnyEncodable("Successfully edited BridgeManager.swift"),
        error: nil
    ),

    ACPToolCallUpdateWire(
        call_id: "call_003",
        status: .completed,
        output: AnyEncodable("Tests passed: 47/47"),
        error: nil
    )
]

// Sample thinking block (agent reasoning)
let sampleThinking = ContentChunk(
    content: .text(TextContent(
        text: """
        I'm refactoring the BridgeManager error handling to use Swift Result types. \
        This will provide better type safety and make error cases explicit. I'll:
        1. Change sendMessage to return Result<String, BridgeError>
        2. Update all call sites to handle .success and .failure cases
        3. Add proper error types for different failure modes
        4. Run tests to ensure no regressions
        """,
        annotations: Annotations(
            audience: ["developers"],
            priority: .normal,
            lastModified: Date()
        )
    ))
)

// Sample agent message
let sampleAgentMessage = ContentChunk(
    content: .text(TextContent(
        text: "I've successfully refactored BridgeManager to use Swift Result types for error handling. All tests pass.",
        annotations: nil
    ))
)
```

### 5. File Change Summary

```swift
struct FileChangeSummary: Codable, Identifiable {
    let id: String  // File path
    let path: String
    let prCount: Int  // How many PRs touched this file
    let totalLinesChanged: Int
    let linesAdded: Int
    let linesRemoved: Int
    let relatedTasks: [String]  // Task IDs
}

let mockFileChanges = [
    FileChangeSummary(
        id: "BridgeManager.swift",
        path: "ios/OpenAgentsCore/Sources/OpenAgentsCore/DesktopBridge/BridgeManager.swift",
        prCount: 3,
        totalLinesChanged: 245,
        linesAdded: 178,
        linesRemoved: 67,
        relatedTasks: ["task_001", "task_003", "task_006"]
    ),

    FileChangeSummary(
        id: "DesktopWebSocketServer.swift",
        path: "ios/OpenAgentsCore/Sources/OpenAgentsCore/DesktopBridge/DesktopWebSocketServer.swift",
        prCount: 2,
        totalLinesChanged: 189,
        linesAdded: 145,
        linesRemoved: 44,
        relatedTasks: ["task_002", "task_006"]
    ),

    FileChangeSummary(
        id: "SessionUpdateHub.swift",
        path: "ios/OpenAgentsCore/Sources/OpenAgentsCore/Agents/SessionUpdateHub.swift",
        prCount: 2,
        totalLinesChanged: 167,
        linesAdded: 98,
        linesRemoved: 69,
        relatedTasks: ["task_003", "task_004"]
    ),

    FileChangeSummary(
        id: "TaskQueue.swift",
        path: "ios/OpenAgentsCore/Sources/OpenAgentsCore/Orchestration/TaskQueue.swift",
        prCount: 1,
        totalLinesChanged: 98,
        linesAdded: 89,
        linesRemoved: 9,
        relatedTasks: ["task_006"]
    )
]
```

---

## Drill-Down Interaction Flows

### Flow 1: PR Detail View

**Trigger**: User taps on a PR card

**macOS View**:
```
┌─────────────────────────────────────────────────────────────────────┐
│ Pull Request #43                                           [×] Close │
├─────────────────────────────────────────────────────────────────────┤
│ Add comprehensive tests for DesktopWebSocketServer                  │
│                                                                     │
│ Status: ⏳ Awaiting Review                     [Approve & Merge]   │
│ Agent: Codex                                   [View on GitHub]    │
│ Duration: 18 minutes                                                │
│ CI Status: ✓ All checks passed                                     │
│                                                                     │
├─────────────────────────────────────────────────────────────────────┤
│ Changes                                                             │
│                                                                     │
│ 5 files changed    +423    -12                                     │
│                                                                     │
│ ● DesktopWebSocketServerTests.swift                    +234  -0    │
│ ● BridgeIntegrationTests.swift                         +98   -0    │
│ ● JSONRPCTests.swift                                   +67   -12   │
│ ● WebSocketConnectionTests.swift                       +24   -0    │
│                                                                     │
│ [View Full Diff]                                                    │
│                                                                     │
├─────────────────────────────────────────────────────────────────────┤
│ Session Activity (12 events)                                        │
│                                                                     │
│ 1:05 AM  🔵 Session started                                        │
│ 1:05 AM  📖 Read DesktopWebSocketServer.swift                      │
│ 1:07 AM  💭 "I'll create comprehensive tests covering connection   │
│              lifecycle, message handling, and error cases..."      │
│ 1:08 AM  ✏️  Edit DesktopWebSocketServerTests.swift               │
│ 1:10 AM  ⚙️  Run: xcodebuild test ...                             │
│ 1:11 AM  ✓  Tests passed: 15/15                                   │
│ 1:12 AM  ✏️  Edit BridgeIntegrationTests.swift                    │
│ 1:14 AM  ⚙️  Run: xcodebuild test ...                             │
│ 1:15 AM  ✓  Tests passed: 23/23                                   │
│ 1:16 AM  💬 "Added 23 comprehensive tests for WebSocket server.   │
│              Coverage increased from 45% to 87%."                  │
│ 1:18 AM  ⚙️  Run: gh pr create ...                                │
│ 1:23 AM  ✓  Session completed                                     │
│                                                                     │
│ [View Full Session Timeline]                                        │
│                                                                     │
├─────────────────────────────────────────────────────────────────────┤
│ Related Task Decision                                               │
│                                                                     │
│ Task: Generate comprehensive tests for DesktopWebSocketServer     │
│ Priority: Medium                                                    │
│ Confidence: 92%                                                     │
│                                                                     │
│ Rationale: "DesktopWebSocketServer modified 18 times with low     │
│ test coverage (current: 45%). Recent sessions show connection     │
│ reliability concerns. Comprehensive tests needed for critical      │
│ bridge infrastructure. Codex selected for its strong test          │
│ generation capabilities."                                           │
│                                                                     │
│ [View Full Decision Analysis]                                       │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

**iOS View**:
```
┌──────────────────────────────────┐
│ < Back              PR #43       │
├──────────────────────────────────┤
│ Add WebSocket tests              │
│                                  │
│ Status: ⏳ Awaiting Review      │
│ Agent: Codex                     │
│ Duration: 18 minutes             │
│ CI: ✓ All checks passed         │
│                                  │
│ ┌──────────────────────────────┐ │
│ │ [Approve & Merge]            │ │
│ │ [View on GitHub]             │ │
│ └──────────────────────────────┘ │
│                                  │
│ Changes (5 files)                │
│ +423 / -12 lines                 │
│                                  │
│ ● DesktopWebSocketServerTests   │
│   +234  -0                       │
│                                  │
│ ● BridgeIntegrationTests         │
│   +98   -0                       │
│                                  │
│ ● JSONRPCTests                   │
│   +67   -12                      │
│                                  │
│ [View Full Diff]                 │
│                                  │
│ Session Activity                 │
│ ┌──────────────────────────────┐ │
│ │ 1:05 AM  🔵 Started          │ │
│ │ 1:05 AM  📖 Read file        │ │
│ │ 1:07 AM  💭 Thinking         │ │
│ │ 1:08 AM  ✏️  Edit file       │ │
│ │ 1:10 AM  ⚙️  Run tests       │ │
│ │ 1:11 AM  ✓  Tests passed     │ │
│ │ [Show All 12 Events]         │ │
│ └──────────────────────────────┘ │
│                                  │
│ Related Decision                 │
│ ┌──────────────────────────────┐ │
│ │ Priority: Medium             │ │
│ │ Confidence: 92%              │ │
│ │                              │ │
│ │ Rationale: "DesktopWebSocket │ │
│ │ Server modified 18 times..." │ │
│ │                              │ │
│ │ [View Full Analysis]         │ │
│ └──────────────────────────────┘ │
│                                  │
└──────────────────────────────────┘
```

### Flow 2: Decision Analysis View

**Trigger**: User taps on a decision card

**macOS View**:
```
┌─────────────────────────────────────────────────────────────────────┐
│ Decision Analysis                                          [×] Close │
├─────────────────────────────────────────────────────────────────────┤
│ Decision: Refactor BridgeManager error handling                    │
│ Made at: 2:30 AM • Friday, Nov 8, 2025                             │
│ Priority: High                                                      │
│ Confidence: 87%  ████████████████████░░░░                          │
│                                                                     │
├─────────────────────────────────────────────────────────────────────┤
│ Task Description                                                    │
│                                                                     │
│ Refactor BridgeManager error handling with Swift Result types      │
│                                                                     │
│ Selected Agent: Claude Code                                         │
│ Estimated Duration: 30 minutes                                      │
│ Actual Duration: 28 minutes                                         │
│                                                                     │
├─────────────────────────────────────────────────────────────────────┤
│ Foundation Models Rationale                                         │
│                                                                     │
│ BridgeManager.swift touched 25 times in recent sessions with user  │
│ frequently requesting error handling improvements. Current          │
│ implementation uses optional returns which masks failure modes.     │
│ Swift Result types provide better error propagation and type        │
│ safety.                                                             │
│                                                                     │
│ High confidence based on:                                           │
│ • Clear user intent extracted from 8 recent sessions                │
│ • Well-scoped task with defined success criteria                    │
│ • File modification frequency indicates active work area            │
│ • Claude Code's strength in Swift refactoring patterns              │
│                                                                     │
│ Risks considered:                                                   │
│ • Breaking changes to existing call sites (medium risk)             │
│ • Potential for test failures (low risk - good test coverage)      │
│                                                                     │
│ Alternative tasks considered:                                       │
│ • "Add logging to bridge components" (lower priority, confidence    │
│   72% - less urgent based on session context)                       │
│ • "Refactor JSON encoding" (lower priority, confidence 65% -        │
│   no strong user signal)                                            │
│                                                                     │
├─────────────────────────────────────────────────────────────────────┤
│ Input Context                                                       │
│                                                                     │
│ Session Analysis:                                                   │
│ • File Frequency:                                                   │
│   - BridgeManager.swift: 25 accesses                                │
│   - DesktopWebSocketServer.swift: 18 accesses                       │
│   - MobileWebSocketClient.swift: 15 accesses                        │
│                                                                     │
│ • Tool Usage:                                                       │
│   - edit_file: 47 calls                                             │
│   - read_file: 25 calls                                             │
│   - run_bash: 15 calls                                              │
│                                                                     │
│ • Extracted User Intent:                                            │
│   "improve error handling and bridge reliability"                  │
│                                                                     │
│ • Recent Goal Patterns:                                             │
│   - "refactor"                                                      │
│   - "error handling"                                                │
│   - "type safety"                                                   │
│                                                                     │
│ • Average Conversation Length: 15.3 events per session              │
│                                                                     │
│ Repository State:                                                   │
│ • Last commit: 2 hours ago                                          │
│ • Branch: main (clean working tree)                                 │
│ • Recent commits focused on bridge improvements                     │
│                                                                     │
├─────────────────────────────────────────────────────────────────────┤
│ Outcome                                                             │
│                                                                     │
│ Status: ✓ Completed Successfully                                   │
│                                                                     │
│ Results:                                                            │
│ • PR #42 created and merged                                         │
│ • 3 files changed (+145 / -67 lines)                                │
│ • All tests passed (47/47)                                          │
│ • Build succeeded on first attempt                                  │
│ • No errors or warnings                                             │
│                                                                     │
│ Evaluation:                                                         │
│ ✓ Task completed within estimated time (28m vs 30m estimated)      │
│ ✓ High code quality (passed all checks)                            │
│ ✓ Successfully merged same day                                     │
│                                                                     │
│ [View Associated PR] [View Session Timeline]                        │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

### Flow 3: Session Timeline Explorer

**Trigger**: User taps "View Full Session Timeline"

**macOS View** (scrollable timeline with filters):
```
┌─────────────────────────────────────────────────────────────────────┐
│ Session Timeline: session_abc123                          [×] Close │
├─────────────────────────────────────────────────────────────────────┤
│ Refactor BridgeManager error handling                              │
│ Claude Code • Started 2:05 AM • Duration: 28 minutes               │
│                                                                     │
│ Filter: [All Events ▼] [Tool Calls] [Messages] [Thinking]         │
│         [Jump to Event #: ___]                                      │
│                                                                     │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│ 2:05:12 AM ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━ │
│ 🔵 Session started                                                 │
│ Mode: claude-code                                                   │
│ Session ID: session_abc123                                          │
│                                                                     │
│ 2:05:34 AM ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━ │
│ 📖 Tool Call: read_file (call_001)                                 │
│ Arguments:                                                          │
│   file_path: "ios/.../BridgeManager.swift"                          │
│ Status: ✓ Completed (0.3s)                                         │
│ Output: "Read 456 lines from BridgeManager.swift"                  │
│ [View File Contents]                                                │
│                                                                     │
│ 2:06:12 AM ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━ │
│ 💭 Agent Thinking                                                  │
│ "I'm refactoring the BridgeManager error handling to use Swift    │
│ Result types. This will provide better type safety and make error  │
│ cases explicit. I'll:                                               │
│ 1. Change sendMessage to return Result<String, BridgeError>        │
│ 2. Update all call sites to handle .success and .failure cases     │
│ 3. Add proper error types for different failure modes              │
│ 4. Run tests to ensure no regressions"                             │
│ [Expand Full Thinking Block]                                        │
│                                                                     │
│ 2:07:45 AM ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━ │
│ ✏️  Tool Call: edit_file (call_002)                                │
│ Arguments:                                                          │
│   file_path: "ios/.../BridgeManager.swift"                          │
│   old_string: "func sendMessage(_ message: BridgeMessage) -> ..."  │
│   new_string: "func sendMessage(_ message: BridgeMessage) -> ..."  │
│ Status: ✓ Completed (0.5s)                                         │
│ Output: "Successfully edited BridgeManager.swift"                  │
│ [View Diff]                                                         │
│                                                                     │
│ 2:09:23 AM ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━ │
│ ✏️  Tool Call: edit_file (call_003)                                │
│ Arguments:                                                          │
│   file_path: "ios/.../BridgeManager.swift"                          │
│   old_string: "guard let result = sendMessage(...) else { ... }"   │
│   new_string: "switch sendMessage(...) { case .success(let ...)"   │
│ Status: ✓ Completed (0.4s)                                         │
│ [View Diff]                                                         │
│                                                                     │
│ ... [15 more events] ...                                            │
│                                                                     │
│ 2:33:05 AM ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━ │
│ 💬 Agent Message                                                   │
│ "I've successfully refactored BridgeManager to use Swift Result    │
│ types for error handling. All tests pass. PR created."             │
│                                                                     │
│ 2:33:18 AM ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━ │
│ ✓ Session completed                                                │
│ Total tool calls: 12                                                │
│ Errors: 0                                                           │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

### Flow 4: Failed Task Detail

**Trigger**: User taps on failed task in Issues section

**macOS View**:
```
┌─────────────────────────────────────────────────────────────────────┐
│ Failed Task: Add SwiftUI previews                         [×] Close │
├─────────────────────────────────────────────────────────────────────┤
│ Status: ⚠️  Failed                                                  │
│ Agent: Codex                                                        │
│ Duration: 10 minutes (3:45 AM - 3:55 AM)                            │
│ Session ID: session_xyz999                                          │
│                                                                     │
├─────────────────────────────────────────────────────────────────────┤
│ Error Details                                                       │
│                                                                     │
│ Build failed after adding previews.                                │
│                                                                     │
│ Error: 'Cannot find ColorScheme in scope'                          │
│ Missing import SwiftUI in 3 files:                                 │
│ • ContentView.swift:23                                              │
│ • SettingsView.swift:18                                             │
│ • AgentListView.swift:15                                            │
│                                                                     │
│ Files Attempted: 8                                                  │
│                                                                     │
├─────────────────────────────────────────────────────────────────────┤
│ Session Log (Last 5 Events)                                         │
│                                                                     │
│ 3:52 AM  ✏️  Edit AgentListView.swift (added preview)              │
│ 3:53 AM  ⚙️  Run: xcodebuild build ...                             │
│ 3:54 AM  ❌ Build failed: 'Cannot find ColorScheme in scope'       │
│ 3:54 AM  💬 "Build failed. Need to add SwiftUI import."            │
│ 3:55 AM  ⚠️  Session failed                                        │
│                                                                     │
│ [View Full Session Log]                                             │
│                                                                     │
├─────────────────────────────────────────────────────────────────────┤
│ Suggested Actions                                                   │
│                                                                     │
│ ┌─────────────────────────────────────────────────────────────────┐ │
│ │ [Retry with Fix Hint]                                           │ │
│ │ Hint: "Add 'import SwiftUI' to all preview files"               │ │
│ └─────────────────────────────────────────────────────────────────┘ │
│                                                                     │
│ ┌─────────────────────────────────────────────────────────────────┐ │
│ │ [Retry Tonight (Same Priority)]                                 │ │
│ └─────────────────────────────────────────────────────────────────┘ │
│                                                                     │
│ ┌─────────────────────────────────────────────────────────────────┐ │
│ │ [Change Agent] Switch to Claude Code for retry                  │ │
│ └─────────────────────────────────────────────────────────────────┘ │
│                                                                     │
│ ┌─────────────────────────────────────────────────────────────────┐ │
│ │ [Cancel Task] Don't retry                                       │ │
│ └─────────────────────────────────────────────────────────────────┘ │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

---

## Interactive Elements & Actions

### Quick Actions (Hero Section)

1. **Approve & Merge All Ready PRs**
   - One-click approval for all PRs with passing CI
   - Confirmation dialog shows list of PRs to merge
   - Sequential merge with status updates

2. **Export Briefing**
   - Formats: Markdown, PDF, HTML
   - Includes: All stats, decisions, PR links, session summaries
   - Shareable report for team visibility

3. **View on GitHub**
   - Opens GitHub repository PR list in browser
- Filtered to show only orchestration-created PRs

4. **Configure Next Run**
   - Quick access to scheduler settings
   - Adjust time window, interval, constraints
   - Preview next wake time

### PR Actions

- **Review PR**: Opens PR detail view (in-app)
- **Approve & Merge**: Direct merge action (with confirmation)
- **View on GitHub**: Opens PR in browser
- **View Related Task**: Navigates to task decision
- **View Session**: Opens session timeline

### Task Actions

- **Retry**: Re-queue failed task for next run
- **Retry with Hint**: Add user guidance for retry
- **Change Agent**: Switch agent for retry
- **Cancel**: Remove from queue
- **View Session**: Open session timeline (if started)
- **View Decision**: Open decision analysis

### Decision Actions

- **View Associated PR**: Navigate to PR detail
- **View Session**: Open session timeline
- **Replay Decision**: Re-run FM analysis with same context
- **Export Analysis**: Save decision rationale as markdown

### Filters & Sorting

**Filter Options**:
- By Agent: Claude Code / Codex / All
- By Status: Completed / Failed / Skipped / All
- By PR Status: Merged / Open / All
- By Priority: High / Medium / Low / All
- By Time: Last Run / Last 7 Days / Last 30 Days / All Time

**Sort Options**:
- By Time (newest first / oldest first)
- By Confidence (highest / lowest)
- By Duration (longest / shortest)
- By Lines Changed (most / least)

### Real-Time Updates (During Active Run)

**Live Progress Indicators**:
- Current task in progress (with progress ring)
- Streaming tool calls from active session
- Next wake time countdown
- Current constraint status (power, network, CPU)

**Notifications**:
- Task completed (with success/failure)
- PR created (with link)
- Build failed (with error preview)
- Run completed (with summary stats)

---

## Platform-Specific Considerations

### macOS: Full Control Center

**Capabilities**:
- Full-featured orchestration control and monitoring
- Rich inline diffs for PR review
- Multi-column layout for parallel information
- Keyboard shortcuts for common actions
- Window management (summary as modal or separate window)
- Drag-and-drop for task priority reordering

**Layout**:
- Sidebar: Quick navigation (Summary, Tasks, PRs, Decisions, Settings)
- Main panel: Detailed content with drill-down
- Inspector panel: Contextual metadata and actions

**Unique Features**:
- AppleScript support for automation
- Share extension for exporting briefings
- Touch Bar integration (if available)
- Menu bar status item for quick access

### iOS: Monitoring-Focused

**Capabilities**:
- Read-only monitoring interface
- Quick PR review and approval
- Tap-to-drill-down navigation
- Share briefing via Messages/Mail/Slack
- Push notifications for orchestration completion

**Layout**:
- Single-column scrolling view
- Card-based UI for scannable summary
- Collapsible sections to reduce scroll depth
- Bottom sheet for drill-down details

**Unique Features**:
- Widgets for Lock Screen / Home Screen (run status, PRs pending)
- Live Activities for active run progress
- Shortcuts integration (e.g., "Show me orchestration PRs")
- Haptic feedback for status changes

**Constraints**:
- No orchestration control (cannot start/stop/configure runs)
- No direct code editing
- Limited PR actions (approve/merge via GitHub API only)

---

## Visual Design Details

### Color Coding

**Status Colors**:
- Completed: `.green` (system green, semantic)
- Failed: `.red` (system red)
- In Progress: `.blue` (system blue)
- Skipped: `.orange` (system orange)
- Pending: `.gray` (system gray)

**Agent Colors**:
- Claude Code: `.purple` (brand color)
- Codex: `.teal` (brand color)

**Priority Colors**:
- High: `.red`
- Medium: `.orange`
- Low: `.gray`

**CI Status Colors**:
- Success: `.green`
- Failure: `.red`
- Pending: `.yellow`

### Typography

**Hierarchy**:
- Hero stats: `.title` (34pt, bold)
- Section headers: `.title2` (22pt, bold)
- Card titles: `.headline` (17pt, semibold)
- Body text: `.body` (17pt, regular)
- Metadata: `.caption` (12pt, regular)
- Monospace: `.system(.body, design: .monospaced)` for code/data

### Spacing

**Consistent padding**:
- Card padding: 16pt
- Section spacing: 24pt
- Inline spacing: 8pt
- Hero section: 32pt

### Animations

**Spring Animations** (for state changes):
- Task status updates: `.spring(response: 0.3, dampingFraction: 0.7)`
- Card expansion: `.spring(response: 0.4, dampingFraction: 0.8)`
- List insertions/removals: `.spring(response: 0.5, dampingFraction: 0.9)`

**Smooth Transitions**:
- Navigation: `.easeInOut(duration: 0.3)`
- Fade in/out: `.opacity` with `.easeIn(duration: 0.2)`

### Materials (Liquid Glass)

**On iOS 26+ / macOS 15+**:
- Background: `.glassEffect(.systemChromeMaterial)`
- Cards: `.glassEffect(.thick, in: .primary)`
- Overlays: `.glassEffect(.ultraThin, in: .primary)`

**Fallback (iOS 16-25 / macOS 13-14)**:
- Background: `.regularMaterial`
- Cards: `.thickMaterial`
- Overlays: `.ultraThinMaterial`

---

## Implementation Notes

### SwiftUI Components Needed

1. **MorningBriefingView.swift** (main view)
   - Consumes `OrchestrationRunSummary` model
   - Renders hero stats, agent performance, PR list, decisions
   - Handles navigation to drill-down views

2. **PRCardView.swift** (PR list item)
   - Displays PR metadata (number, title, status, agent, timing)
   - Actions: Review, Approve & Merge, View on GitHub
   - Status badges with color coding

3. **DecisionCardView.swift** (decision list item)
   - Shows decision summary (task, agent, priority, confidence)
   - Expandable to show full rationale
   - Navigation to full decision analysis

4. **AgentPerformanceView.swift** (agent breakdown section)
   - Horizontal progress bars for agent task distribution
   - Stats: task count, average duration, success rate
   - Visual comparison between agents

5. **PRDetailView.swift** (drill-down)
   - Full PR metadata
   - File change list
   - Session activity timeline
   - Related decision
   - Actions: Approve, Merge, View on GitHub

6. **DecisionAnalysisView.swift** (drill-down)
   - Complete FM rationale
   - Input context (file frequency, tool usage, user intent)
   - Confidence breakdown
   - Outcome evaluation
   - Links to PR and session

7. **SessionTimelineView.swift** (drill-down)
   - Scrollable timeline of ACP SessionUpdate events
   - Filters: All, Tool Calls, Messages, Thinking
   - Expandable event details
   - Jump to event number

8. **FailedTaskDetailView.swift** (drill-down)
   - Error details
   - Session log
   - Suggested actions: Retry, Retry with Hint, Change Agent, Cancel

### State Management

**ObservableObject Pattern**:
```swift
@MainActor
class MorningBriefingViewModel: ObservableObject {
    @Published var summary: OrchestrationRunSummary?
    @Published var tasks: [OrchestrationTask] = []
    @Published var prs: [PRSummary] = []
    @Published var isLoading: Bool = false
    @Published var error: Error?

    // Filters
    @Published var selectedAgent: ACPSessionModeId?
    @Published var selectedStatus: TaskStatus?
    @Published var selectedPRStatus: PRStatus?

    func loadLatestRun() async {
        // Fetch from TaskQueue + PRAutomationService
    }

    func approveAndMergePR(_ pr: PRSummary) async throws {
        // Use gh CLI via PRAutomationService
    }

    func retryTask(_ task: OrchestrationTask) async {
        // Re-queue in TaskQueue
    }

    func exportBriefing(format: ExportFormat) async throws -> URL {
        // Generate markdown/PDF/HTML
    }
}
```

### Data Flow

1. **On View Appear**:
   - `MorningBriefingView` creates `MorningBriefingViewModel`
- ViewModel fetches latest orchestration run from `TaskQueue`
   - Loads associated PR data from `PRAutomationService`
   - Populates summary, tasks, PRs arrays
   - View renders with mock or real data

2. **User Interaction**:
   - User taps PR card → Navigate to `PRDetailView`
   - User taps "Approve & Merge" → Call `approveAndMergePR()`
   - User taps decision card → Navigate to `DecisionAnalysisView`
   - User taps failed task → Navigate to `FailedTaskDetailView`

3. **Real-Time Updates** (if run in progress):
   - ViewModel subscribes to `TaskQueue.updates` AsyncStream
   - Updates `tasks` array when new events arrive
   - SwiftUI automatically re-renders affected views

### Navigation Pattern

**macOS**:
```swift
NavigationSplitView {
    // Sidebar
    List(selection: $selectedSection) {
        NavigationLink("Summary", value: Section.summary)
        NavigationLink("Tasks", value: Section.tasks)
        NavigationLink("PRs", value: Section.prs)
        NavigationLink("Decisions", value: Section.decisions)
    }
} detail: {
    // Main content based on selection
    switch selectedSection {
    case .summary:
        MorningBriefingView(viewModel: viewModel)
    case .tasks:
        TaskListView(tasks: viewModel.tasks)
    // ...
    }
}
```

**iOS**:
```swift
NavigationStack {
    MorningBriefingView(viewModel: viewModel)
        .navigationTitle("Morning Briefing")
        .navigationBarTitleDisplayMode(.large)
}
.sheet(item: $selectedPR) { pr in
    PRDetailView(pr: pr, viewModel: prViewModel)
}
.sheet(item: $selectedDecision) { decision in
    DecisionAnalysisView(decision: decision)
}
```

### Accessibility

- All interactive elements have `.accessibilityLabel()`
- Status badges use `.accessibilityValue()` for screen reader context
- Charts/visualizations have `.accessibilityChartDescriptor()`
- VoiceOver navigation order follows visual hierarchy
- Dynamic Type support for all text (no hardcoded font sizes)
- Color contrast meets WCAG AA standards (4.5:1 minimum)

### Testing Strategy

**Unit Tests**:
- `MorningBriefingViewModel` logic
- Data transformations (raw ACP → UI models)
- Filter/sort operations
- Mock data generation

**Integration Tests**:
- End-to-end flow: Load run → Display summary → Drill down → Navigate back
- PR approval flow (mock GitHub API)
- Task retry flow (mock TaskQueue)

**UI Tests**:
- Hero stats rendering
- PR list scrolling and tapping
- Decision card expansion
- Filter/sort controls
- Navigation to drill-down views

**Snapshot Tests** (optional):
- Visual regression testing for card layouts
- Light/dark mode variants
- Different screen sizes (iPhone SE to iPad Pro, macOS window sizes)

---

## Success Criteria

This morning briefing screen is successful if:

1. **Glanceable** (< 5 seconds to understand what happened overnight)
2. **Actionable** (clear CTAs for PR review/merge)
3. **Transparent** (full visibility into FM decisions and agent execution)
4. **Drill-downable** (can explore to any level of detail desired)
5. **Celebratory** (feels rewarding to see hours of autonomous work completed)
6. **Platform-appropriate** (full control on macOS, monitoring on iOS)
7. **ACP-compliant** (all data structures follow Agent Client Protocol)

---

## Next Steps

1. **Review this mockup** with stakeholders
2. **Create Swift prototypes** of key components (hero stats, PR card, decision card)
3. **Test with real overnight run data** (when available)
4. **Iterate on information hierarchy** based on user testing
5. **Implement drill-down views** incrementally
6. **Add animations and polish** for final release

---

## Open Questions

1. Should we show git diff inline or require jumping to GitHub?
2. How much of the FM rationale should be visible by default vs expandable?
3. Should failed tasks automatically retry next run, or require explicit user action?
4. What's the retention policy for overnight briefings (keep last 30 days? all time?)
5. Should iOS have any orchestration control, or stay monitoring-only?
6. How do we handle partial runs (e.g., user wakes up at 3am, run still in progress)?
7. Should we generate AI-powered summary of overnight work (meta-summary)?

---

## Completion Summary

**Completed Deliverables**:

### 1. ✅ Comprehensive Mockup Documentation
- Full UI layouts for macOS and iOS
- Complete ACP-shaped mock data structures
- Drill-down interaction flows
- Implementation notes and component specifications
- This document (1,627 lines)

### 2. ✅ Demo UI Implementation
**Files Created**:
- `ios/OpenAgents/MockData/OrchestrationMockData.swift` - Realistic mock data
- `ios/OpenAgents/Views/MorningBriefingDemoView.swift` - Main demo view
- `ios/OpenAgents/Views/Components/MorningBriefingStatsView.swift` - Hero stats
- `ios/OpenAgents/Views/Components/AgentPerformanceView.swift` - Agent breakdown
- `ios/OpenAgents/Views/Components/PRCardView.swift` - PR cards
- `ios/OpenAgents/Views/Components/DecisionCardView.swift` - FM decision cards
- `ios/OpenAgents/SimplifiedMacOSView.swift` - Navigation integration

**UI Features**:
- Full-screen navigation (not modal)
- Hero stats showing decisions, tasks completed, PRs created
- Agent performance breakdown with progress bars
- PR cards with status, metadata, diff stats
- Expandable FM decision cards with confidence indicators
- Ready for demo and user testing

### 3. ✅ Phase One Backend Infrastructure (Commit: b05dde8e)
**Files Created**:
- `ios/OpenAgentsCore/Sources/OpenAgentsCore/Orchestration/DecisionEngine.swift` (197 lines)
  - Heuristic-based task decision logic
  - Session analysis integration
  - Time budget clamping, confidence scoring
  - Refactor vs tests decision paths

- `ios/OpenAgentsCore/Sources/OpenAgentsCore/Orchestration/TaskQueue.swift` (344 lines)
  - SQLite-persisted task queue with actor isolation
  - OpHash deduplication
  - FIFO ordering, status transitions
  - Full CRUD operations

- `ios/OpenAgentsCore/Tests/OpenAgentsCoreTests/Orchestration/DecisionEngineTests.swift` (241 lines)
  - 10 comprehensive test cases
  - TDD approach

- `ios/OpenAgentsCore/Tests/OpenAgentsCoreTests/Orchestration/TaskQueueTests.swift` (359 lines)
  - 14 comprehensive test cases
  - Persistence, deduplication, status transitions

- `ios/OpenAgentsCore/Sources/OpenAgentsCore/Tinyvex/DbLayer.swift` (modified)
  - Added public SQL helper methods

**Test Coverage**: All tests passing, build succeeds

### 4. ✅ Phase Two Backend Infrastructure (Commit: c98e762e)
**Files Created**:
- `ios/OpenAgentsCore/Sources/OpenAgentsCore/Orchestration/AgentCoordinator.swift` (313 lines)
  - Core orchestration loop actor
  - runCycle(), completeTask(), cancelTask(), metrics()
  - Integration with DecisionEngine, TaskQueue, AgentRegistry

- `ios/OpenAgentsCore/Tests/OpenAgentsCoreTests/Orchestration/AgentCoordinatorTests.swift` (394 lines)
  - 8 comprehensive test cases
  - Mock agent provider for isolated testing

**Integration**: Fully integrated with existing AgentProvider, AgentRegistry, SessionUpdateHub

**Test Coverage**: All tests passing, build succeeds

---

**Document Status**: ✅ Completed and implemented
**Last Updated**: Nov 8, 2025
**Author**: AI Agent (Claude Code)
**Implementation**: Demo UI + Full backend infrastructure (phases 1 & 2)
**Commits**: b05dde8e (phase 1), c98e762e (phase 2)
> Reopen note (2025-11-08): Reopened to implement integration updates: inject SessionUpdateHub into AgentCoordinator, add runtime timeout cancellation, adopt stable opHash, and align naming to Orchestration across docs. This note will be replaced with a closure summary upon completion.
