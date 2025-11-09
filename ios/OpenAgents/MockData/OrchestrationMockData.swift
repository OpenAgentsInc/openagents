import Foundation
import OpenAgentsCore

// MARK: - Mock Data Structures

enum TaskStatus: String, Codable {
    case pending
    case in_progress
    case completed
    case failed
    case cancelled
    case skipped
}

enum Priority: String, Codable {
    case high
    case medium
    case low
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

struct AgentStats: Codable {
    let tasksCompleted: Int
    let averageDuration: TimeInterval
    let successRate: Double
    let toolCallsTotal: Int
}

struct OvernightRunSummary: Codable {
    let runId: String
    let startTime: Date
    let endTime: Date
    let duration: TimeInterval
    let totalCycles: Int
    let completedTasks: Int
    let failedTasks: Int
    let skippedTasks: Int
    let cancelledTasks: Int
    let prsCreated: Int
    let prsMerged: Int
    let prsAwaitingReview: Int
    let prsFailed: Int
    let autonomousWorkTime: TimeInterval
    let idleTime: TimeInterval
    let filesChanged: Int
    let linesAdded: Int
    let linesRemoved: Int
    let toolCallsTotal: Int
    let toolCallsByType: [String: Int]
    let agentBreakdown: [String: AgentStats]
}

struct TaskDecision: Codable {
    let task: String
    let agent: ACPSessionModeId
    let priority: Priority
    let estimatedDuration: TimeInterval
    let rationale: String
    let confidence: Double
}

struct OrchestrationTask: Codable, Identifiable {
    let id: String
    let opHash: String
    var status: TaskStatus
    let decision: TaskDecision
    var sessionId: String?
    let createdAt: Date
    var startedAt: Date?
    var completedAt: Date?
    var error: String?
    let metadata: [String: String]
}

struct PRSummary: Codable, Identifiable {
    let id: Int
    let number: Int
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
    let taskId: String
    let duration: TimeInterval
    let ciStatus: CIStatus?
}

// MARK: - Mock Data

enum MockOrchestrationData {

    static let overnightRun = OvernightRunSummary(
        runId: "run_20251108_overnight",
        startTime: Date(timeIntervalSince1970: 1731024000),  // Nov 8, 2025 1:00 AM
        endTime: Date(timeIntervalSince1970: 1731038400),    // Nov 8, 2025 5:00 AM
        duration: 14400,  // 4 hours
        totalCycles: 8,
        completedTasks: 6,
        failedTasks: 1,
        skippedTasks: 1,
        cancelledTasks: 0,
        prsCreated: 6,
        prsMerged: 1,
        prsAwaitingReview: 5,
        prsFailed: 0,
        autonomousWorkTime: 11520,  // 3.2 hours
        idleTime: 2880,  // 48 minutes
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
            "claude_code": AgentStats(
                tasksCompleted: 4,
                averageDuration: 1680,  // 28 minutes
                successRate: 1.0,
                toolCallsTotal: 58
            ),
            "codex": AgentStats(
                tasksCompleted: 2,
                averageDuration: 1080,  // 18 minutes
                successRate: 0.67,
                toolCallsTotal: 29
            )
        ]
    )

    static let tasks: [OrchestrationTask] = [
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
                confidence: 0.87
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
                "branch": "agent/orchestration/bridge-error-handling",
                "config_id": "nightly",
                "goals_hash": "8a7f3c91e5d2",
                "goals": "refactor error handling, improve code quality"
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
                confidence: 0.92
            ),
            sessionId: "session_def456",
            createdAt: Date(timeIntervalSince1970: 1731024000),
            startedAt: Date(timeIntervalSince1970: 1731024300),
            completedAt: Date(timeIntervalSince1970: 1731025380),
            metadata: [
                "pr_number": "43",
                "pr_url": "https://github.com/OpenAgentsInc/openagents/pull/43",
                "pr_status": "open",
                "files_changed": "5",
                "lines_added": "423",
                "lines_removed": "12",
                "branch": "agent/orchestration/websocket-tests",
                "config_id": "nightly",
                "goals_hash": "8a7f3c91e5d2",
                "goals": "refactor error handling, improve code quality"
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
                confidence: 0.78
            ),
            sessionId: "session_ghi789",
            createdAt: Date(timeIntervalSince1970: 1731022800),
            startedAt: Date(timeIntervalSince1970: 1731023100),
            completedAt: Date(timeIntervalSince1970: 1731025020),
            metadata: [
                "pr_number": "44",
                "pr_url": "https://github.com/OpenAgentsInc/openagents/pull/44",
                "pr_status": "open",
                "files_changed": "2",
                "lines_added": "98",
                "lines_removed": "145",
                "branch": "agent/orchestration/concurrency-refactor",
                "config_id": "weekly",
                "goals_hash": "4b9e2d7f1c8a",
                "goals": "increase test coverage, add documentation"
            ]
        ),

        OrchestrationTask(
            id: "task_004",
            opHash: "hash_fm_caching",
            status: .completed,
            decision: TaskDecision(
                task: "Add Foundation Models caching layer",
                agent: .claude_code,
                priority: .medium,
                estimatedDuration: 1500,
                rationale: """
                Foundation Models calls are repeated for the same prompts, wasting \
                processing time. Implementing a caching layer will improve performance \
                and reduce redundant computation. Clear performance win.
                """,
                confidence: 0.85
            ),
            sessionId: "session_jkl012",
            createdAt: Date(timeIntervalSince1970: 1731026700),
            startedAt: Date(timeIntervalSince1970: 1731027000),
            completedAt: Date(timeIntervalSince1970: 1731028200),
            metadata: [
                "pr_number": "45",
                "pr_url": "https://github.com/OpenAgentsInc/openagents/pull/45",
                "pr_status": "open",
                "files_changed": "4",
                "lines_added": "234",
                "lines_removed": "45",
                "branch": "agent/orchestration/fm-caching"
            ]
        ),

        OrchestrationTask(
            id: "task_005",
            opHash: "hash_api_docs",
            status: .completed,
            decision: TaskDecision(
                task: "Generate API documentation from code",
                agent: .codex,
                priority: .low,
                estimatedDuration: 900,
                rationale: """
                Many public APIs lack documentation comments. Automated generation \
                from code signatures and context will improve developer experience. \
                Codex excels at documentation generation.
                """,
                confidence: 0.75
            ),
            sessionId: "session_mno345",
            createdAt: Date(timeIntervalSince1970: 1731030900),
            startedAt: Date(timeIntervalSince1970: 1731031200),
            completedAt: Date(timeIntervalSince1970: 1731031800),
            metadata: [
                "pr_number": "46",
                "pr_url": "https://github.com/OpenAgentsInc/openagents/pull/46",
                "pr_status": "open",
                "files_changed": "8",
                "lines_added": "567",
                "lines_removed": "23",
                "branch": "agent/orchestration/api-docs"
            ]
        ),

        OrchestrationTask(
            id: "task_006",
            opHash: "hash_taskqueue_optimization",
            status: .completed,
            decision: TaskDecision(
                task: "Optimize TaskQueue database queries",
                agent: .claude_code,
                priority: .medium,
                estimatedDuration: 1320,
                rationale: """
                TaskQueue shows slow query performance with N+1 query patterns. \
                Optimizing with proper indexing and batch queries will improve \
                orchestration responsiveness. Performance profiling confirms this.
                """,
                confidence: 0.82
            ),
            sessionId: "session_pqr678",
            createdAt: Date(timeIntervalSince1970: 1731034080),
            startedAt: Date(timeIntervalSince1970: 1731034380),
            completedAt: Date(timeIntervalSince1970: 1731035400),
            metadata: [
                "pr_number": "47",
                "pr_url": "https://github.com/OpenAgentsInc/openagents/pull/47",
                "pr_status": "open",
                "files_changed": "2",
                "lines_added": "89",
                "lines_removed": "156",
                "branch": "agent/orchestration/taskqueue-optimization"
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
                confidence: 0.65
            ),
            sessionId: "session_xyz999",
            createdAt: Date(timeIntervalSince1970: 1731033600),
            startedAt: Date(timeIntervalSince1970: 1731033900),
            completedAt: Date(timeIntervalSince1970: 1731034500),
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
                confidence: 0.55
            ),
            sessionId: nil,
            createdAt: Date(timeIntervalSince1970: 1731038100),
            startedAt: nil,
            completedAt: nil,
            metadata: [
                "skip_reason": "Time budget exceeded (4hr limit reached)",
                "reschedule": "next_run"
            ]
        )
    ]

    static let prs: [PRSummary] = [
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
}
