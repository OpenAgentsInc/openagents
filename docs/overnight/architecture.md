# Overnight Agents - Technical Architecture

**Last Updated**: 2025-11-08
**Status**: Design Phase
**Target Platform**: macOS 26.0+ (execution), iOS 16.0+ (monitoring)

## Table of Contents

1. [Design Principles](#design-principles)
2. [System Architecture](#system-architecture)
3. [Component Details](#component-details)
4. [Data Flow](#data-flow)
5. [State Management](#state-management)
6. [Error Handling](#error-handling)
7. [Security & Safety](#security--safety)
8. [Performance Considerations](#performance-considerations)
9. [Integration Points](#integration-points)

---

## Design Principles

### 1. **Apple-First Architecture**
- On-device Foundation Models for all decision-making (privacy-first, no cloud dependencies)
- macOS-only execution (iOS = monitoring UI via WebSocket bridge)
- Native Swift with async/await concurrency
- IOKit integration for power/idle state detection
- Compliant with App Store Guidelines and DPLA §3.3.8

### 2. **ACP-Native Throughout**
- All agent communication via Agent Client Protocol (ADR-0002)
- No proprietary JSONL or custom formats at system boundaries
- SessionUpdate streaming from agents → orchestrator → UI
- Tool call lifecycle visible in real-time

### 3. **Deterministic & Reproducible**
- Upgrade manifests are pure JSON (no hidden state)
- Pipeline execution is idempotent where possible
- Deduplication via opHash (same operation = same hash)
- Observable state for debugging and auditing

### 4. **Safety & Constraints**
- Foundation Models AUP compliance checks before execution
- Resource limits (CPU, memory, disk, time)
- Filesystem permissions (whitelist/blacklist)
- User-configurable guardrails

### 5. **Future-Ready**
- Upgrade manifest format designed for Nostr transfer (kind 30051)
- Payment/licensing hooks for Spark SDK integration
- Reputation scoring hooks (kind 30054)
- P2P coordination primitives (future: federated mesh)

---

## System Architecture

### High-Level Components

```
┌────────────────────────────────────────────────────────────────────────┐
│                         macOS Desktop Layer                            │
├────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  ┌──────────────────────────────────────────────────────────────────┐  │
│  │                    Orchestration Layer                           │  │
│  ├──────────────────────────────────────────────────────────────────┤  │
│  │                                                                   │  │
│  │  ┌─────────────────┐         ┌──────────────────────────┐        │  │
│  │  │ SchedulerService│────────▶│ UpgradeExecutor          │        │  │
│  │  │                 │         │ - Load manifest          │        │  │
│  │  │ - Timer loop    │         │ - Validate ops           │        │  │
│  │  │ - Constraints   │         │ - Execute pipeline       │        │  │
│  │  │ - Jitter        │         └──────────────────────────┘        │  │
│  │  └─────────────────┘                    │                        │  │
│  │         │                                │                        │  │
│  │         └────────────────┬───────────────┘                        │  │
│  │                          ▼                                        │  │
│  │         ┌─────────────────────────────────────────┐              │  │
│  │         │    DecisionOrchestrator                 │              │  │
│  │         │  ┌───────────────────────────────────┐  │              │  │
│  │         │  │ Foundation Models Integration     │  │              │  │
│  │         │  │ - LanguageModelSession            │  │              │  │
│  │         │  │ - Native tool calling loop        │  │              │  │
│  │         │  │ - session.* tools (history)       │  │              │  │
│  │         │  │ - repo.* tools (git status, etc.) │  │              │  │
│  │         │  └───────────────────────────────────┘  │              │  │
│  │         │                                          │              │  │
│  │         │  Input:  OrchestrationContext           │              │  │
│  │         │  Output: TaskDecision                   │              │  │
│  │         └─────────────────────────────────────────┘              │  │
│  │                          │                                        │  │
│  │                          ▼                                        │  │
│  │         ┌─────────────────────────────────────────┐              │  │
│  │         │         TaskQueue                       │              │  │
│  │         │  - SQLite persistence (Tinyvex)         │              │  │
│  │         │  - Priority scheduling                  │              │  │
│  │         │  - Status lifecycle tracking            │              │  │
│  │         │  - Deduplication (opHash)               │              │  │
│  │         └─────────────────────────────────────────┘              │  │
│  │                          │                                        │  │
│  │                          ▼                                        │  │
│  │         ┌─────────────────────────────────────────┐              │  │
│  │         │     PolicyEnforcer                      │              │  │
│  │         │  - AUP compliance check                 │              │  │
│  │         │  - Resource limit validation            │              │  │
│  │         │  - Time budget enforcement              │              │  │
│  │         └─────────────────────────────────────────┘              │  │
│  │                          │                                        │  │
│  └──────────────────────────┼────────────────────────────────────────┘  │
│                             ▼                                           │
│  ┌──────────────────────────────────────────────────────────────────┐  │
│  │                    Agent Execution Layer                         │  │
│  ├──────────────────────────────────────────────────────────────────┤  │
│  │                                                                   │  │
│  │  ┌─────────────────────────────────────────────────────────────┐ │  │
│  │  │            AgentCoordinator                                  │ │  │
│  │  │  - Delegates tasks to AgentProvider                          │ │  │
│  │  │  - Monitors ACP SessionUpdate stream                         │ │  │
│  │  │  - Handles concurrent sessions                               │ │  │
│  │  │  - Resume on error with context                              │ │  │
│  │  └─────────────────────────────────────────────────────────────┘ │  │
│  │                             │                                     │  │
│  │           ┌─────────────────┴──────────────────┐                 │  │
│  │           ▼                                     ▼                 │  │
│  │  ┌──────────────────┐                ┌──────────────────┐        │  │
│  │  │ ClaudeCodeAgent  │                │  CodexAgent      │        │  │
│  │  │ Provider         │                │  Provider        │        │  │
│  │  │                  │                │                  │        │  │
│  │  │ - Process mgmt   │                │ - Process mgmt   │        │  │
│  │  │ - Text → ACP     │                │ - JSONL → ACP    │        │  │
│  │  │ - Session resume │                │ - Thread resume  │        │  │
│  │  └──────────────────┘                └──────────────────┘        │  │
│  │           │                                     │                 │  │
│  └───────────┼─────────────────────────────────────┼─────────────────┘  │
│              │                                     │                    │
│              └─────────────┬───────────────────────┘                    │
│                            ▼                                            │
│  ┌──────────────────────────────────────────────────────────────────┐  │
│  │                 SessionUpdateHub                                 │  │
│  │  - Broadcast ACP updates to all subscribers                      │  │
│  │  - Bridge to iOS via WebSocket                                   │  │
│  │  - Tinyvex persistence                                           │  │
│  └──────────────────────────────────────────────────────────────────┘  │
│                            │                                            │
│                            ▼                                            │
│  ┌──────────────────────────────────────────────────────────────────┐  │
│  │                GitHub Integration Layer                          │  │
│  ├──────────────────────────────────────────────────────────────────┤  │
│  │                                                                   │  │
│  │  ┌──────────────────────────────────────────────────────────────┐│  │
│  │  │         PRAutomationService                                   ││  │
│  │  │  - Branch management (agent/session-{id})                     ││  │
│  │  │  - Commit generation from ACP tool calls                      ││  │
│  │  │  - gh CLI integration (auth via system keychain)              ││  │
│  │  │  - PR creation with template                                  ││  │
│  │  │  - Auto-push or manual approval                               ││  │
│  │  └──────────────────────────────────────────────────────────────┘│  │
│  └──────────────────────────────────────────────────────────────────┘  │
│                            │                                            │
└────────────────────────────┼────────────────────────────────────────────┘
                             │
                             │ (DesktopWebSocketServer)
                             │ JSON-RPC 2.0 over WebSocket
                             ▼
┌────────────────────────────────────────────────────────────────────────┐
│                          iOS Monitoring Layer                          │
├────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  ┌──────────────────────────────────────────────────────────────────┐  │
│  │              OvernightMonitoringView                             │  │
│  │  ┌────────────────┐  ┌────────────────┐  ┌────────────────────┐ │  │
│  │  │ TaskQueueCard  │  │ SessionCard    │  │ DecisionCard       │ │  │
│  │  │ - Pending: 3   │  │ Claude Code    │  │ "Refactor error    │ │  │
│  │  │ - Active: 1    │  │ In progress    │  │  handling..."      │ │  │
│  │  │ - Done: 5      │  │ 47 tool calls  │  │ Rationale: ...     │ │  │
│  │  └────────────────┘  └────────────────┘  └────────────────────┘ │  │
│  │                                                                   │  │
│  │  ┌────────────────────────────────────────────────────────────┐  │  │
│  │  │ PRPreviewCard                                               │  │
│  │  │ PR #42: "Refactor BridgeManager error handling"            │  │
│  │  │ [Approve] [Edit] [Cancel]                                  │  │
│  │  └────────────────────────────────────────────────────────────┘  │  │
│  └──────────────────────────────────────────────────────────────────┘  │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

---

## Component Details

### 1. SchedulerService

**Responsibility**: Time-based orchestration wake-up with constraint checking

**Location**: `ios/OpenAgentsCore/Sources/OpenAgentsCore/Orchestration/SchedulerService.swift`

**Interface**:
```swift
actor SchedulerService {
    /// Start scheduler with upgrade manifest
    func start(upgrade: UpgradeManifest) async throws

    /// Stop scheduler gracefully (finish current task)
    func stop() async

    /// Check if constraints are satisfied
    func checkConstraints(_ constraints: ScheduleConstraints) async -> Bool

    /// Calculate next wake time with jitter
    func nextWakeTime(from: Date, cron: String, jitter: Int) -> Date?

    /// Current scheduler state
    var state: SchedulerState { get async }
}

enum SchedulerState {
    case idle
    case running(nextWake: Date)
    case paused(reason: String)
    case stopped
}
```

**Key Features**:
- Cron expression parsing (5-field: minute hour day month weekday)
- Time window enforcement (e.g., 01:00-05:00)
- Jitter support (random delay 0-N seconds)
- Constraint checking:
  - `plugged_in`: IOPMCopyBatteryInfo (on battery vs AC)
  - `wifi_only`: NWPathMonitor (cellular vs wifi)
  - `cpu_max_percentage`: host_processor_info (CPU usage)
  - `respect_dnd`: DistributedNotificationCenter (com.apple.donotdisturb)
  - `suspend_if_active`: NSWorkspace.shared.frontmostApplication
- Catch-up policy: `run_once_at_next_opportunity` or `skip`

**Implementation Notes**:
- Use `Timer.publish()` or `AsyncStream<Date>` for timer loop
- All constraint checks are async and cancellable
- State is observable via `@Published` or AsyncSequence
- Integrates with UpgradeExecutor for pipeline execution

**Dependencies**:
- IOKit (power state)
- Network (network type)
- NSWorkspace (user activity)
- UpgradeExecutor

---

### 2. DecisionOrchestrator

**Responsibility**: Foundation Models-powered task selection and agent assignment

**Location**: `ios/OpenAgentsCore/Sources/OpenAgentsCore/Orchestration/DecisionOrchestrator.swift`

**Interface**:
```swift
actor DecisionOrchestrator {
    /// Decide next task based on context
    func decideNextTask(context: OrchestrationContext) async throws -> TaskDecision

    /// Analyze session history for patterns
    func analyzeSessionHistory(providers: [AgentType], topK: Int) async throws -> [SessionInsight]

    /// Get repository status (git status, recent commits, etc.)
    func getRepoStatus(workingDir: URL) async throws -> RepoStatus

    /// Prioritize candidate tasks
    func prioritizeTasks(_ candidates: [TaskCandidate]) async throws -> [TaskDecision]
}

struct OrchestrationContext {
    let sessionInsights: [SessionInsight]
    let repoStatus: RepoStatus
    let recentTasks: [OvernightTask]
    let availableAgents: [AgentType]
    let timeBudget: TimeInterval
    let userPreferences: [String: String]
}

struct TaskDecision {
    let task: String  // Natural language task description
    let agent: AgentType  // .claude_code or .codex
    let priority: Priority  // high, medium, low
    let estimatedDuration: TimeInterval
    let rationale: String  // FM explanation
    let confidence: Double  // 0.0-1.0
    let metadata: [String: String]
}

struct SessionInsight {
    let provider: AgentType
    let fileFrequency: [String: Int]  // File path → touch count
    let toolUsage: [String: Int]  // Tool name → usage count
    let userIntents: [String]  // Inferred intents (refactor, test, debug, etc.)
    let avgSessionDuration: TimeInterval
}

struct RepoStatus {
    let branch: String
    let ahead: Int
    let behind: Int
    let modifiedFiles: [String]
    let untrackedFiles: [String]
    let recentCommits: [GitCommit]
    let testCoverage: Double?  // 0.0-1.0 if available
}
```

**Key Features**:
- Uses `FMOrchestrator` (existing) with custom tools:
  - `session.analyze` (existing): Read session history
  - `repo.status` (new): Git repository state
  - `repo.coverage` (new): Parse test coverage reports
  - `repo.complexity` (new): Code complexity metrics
- Prompt template:
  ```
  You are an autonomous code quality agent deciding what to work on next.

  Context:
  - Recent sessions: {sessionInsights}
  - Repository state: {repoStatus}
  - Recent tasks: {recentTasks}
  - Time budget: {timeBudget}

  Available agents:
  - claude-code: Best for refactoring, documentation, complex reasoning
  - codex: Best for test generation, boilerplate, repetitive tasks

  Decide the highest-impact task to work on right now. Consider:
  1. Files touched frequently (need refactoring)
  2. Low test coverage areas (need tests)
  3. User intents from past sessions (what they care about)
  4. Time budget (can we finish in {timeBudget}?)

  Return JSON:
  {
    "task": "Clear, specific task description",
    "agent": "claude-code" or "codex",
    "priority": "high" | "medium" | "low",
    "estimated_duration": seconds,
    "rationale": "Why this task now?"
  }
  ```

**Implementation Notes**:
- Check FM availability: `SystemLanguageModel.default.availability`
- Use streaming for responsiveness (show thinking process)
- Cache session insights (expensive to recompute every time)
- Fallback to heuristic if FM unavailable (file frequency + random selection)

**Dependencies**:
- FMOrchestrator (existing)
- SessionHistoryTools (existing)
- Git (via Process)
- AgentRegistry

---

### 3. TaskQueue

**Responsibility**: Persistent work queue with status tracking

**Location**: `ios/OpenAgentsCore/Sources/OpenAgentsCore/Orchestration/TaskQueue.swift`

**Interface**:
```swift
actor TaskQueue {
    /// Add task to queue
    func enqueue(_ task: OvernightTask) async throws -> TaskID

    /// Get next pending task
    func dequeue() async throws -> OvernightTask?

    /// Update task status
    func updateStatus(_ taskId: TaskID, status: TaskStatus) async throws

    /// Get all tasks (optionally filtered)
    func all(filter: TaskFilter?) async throws -> [OvernightTask]

    /// Remove completed tasks older than N days
    func cleanup(olderThan: TimeInterval) async throws

    /// Observable stream of queue changes
    var updates: AsyncStream<TaskQueueUpdate> { get }
}

struct OvernightTask: Codable, Identifiable {
    let id: TaskID  // UUID
    let opHash: String  // SHA256 of task details (deduplication)
    var status: TaskStatus
    let decision: TaskDecision
    var sessionId: String?  // Set when agent starts
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
}

struct TaskFilter {
    var status: TaskStatus?
    var agent: AgentType?
    var priority: Priority?
    var createdAfter: Date?
}

enum TaskQueueUpdate {
    case enqueued(TaskID)
    case statusChanged(TaskID, TaskStatus)
    case dequeued(TaskID)
}
```

**Implementation**:
- SQLite via Tinyvex (append-only log + materialized view)
- Table schema:
  ```sql
  CREATE TABLE overnight_tasks (
      id TEXT PRIMARY KEY,
      op_hash TEXT NOT NULL,
      status TEXT NOT NULL,
      decision_json TEXT NOT NULL,
      session_id TEXT,
      created_at INTEGER NOT NULL,
      started_at INTEGER,
      completed_at INTEGER,
      error TEXT,
      metadata_json TEXT
  );

  CREATE INDEX idx_status ON overnight_tasks(status);
  CREATE INDEX idx_created_at ON overnight_tasks(created_at);
  CREATE INDEX idx_op_hash ON overnight_tasks(op_hash);
  ```

**Deduplication Logic**:
```swift
func opHash(_ decision: TaskDecision) -> String {
    let canonical = "\(decision.task)|\(decision.agent)|\(decision.priority)"
    return SHA256.hash(data: canonical.data(using: .utf8)!).hexString
}

// Before enqueue:
if let existing = await taskQueue.all().first(where: { $0.opHash == hash && $0.status != .failed }) {
    // Already queued or completed, skip
    return existing.id
}
```

**Dependencies**:
- Tinyvex (SQLite wrapper)
- Combine (for observable updates)

---

### 4. AgentCoordinator

**Responsibility**: Delegate tasks to agents and monitor progress

**Location**: `ios/OpenAgentsCore/Sources/OpenAgentsCore/Orchestration/AgentCoordinator.swift`

**Interface**:
```swift
actor AgentCoordinator {
    /// Delegate task to appropriate agent
    func delegate(_ task: OvernightTask) async throws -> AgentSessionResult

    /// Monitor ongoing session (stream ACP updates)
    func monitorSession(_ sessionId: String) -> AsyncStream<ACPSessionUpdate>

    /// Cancel running session
    func cancelSession(_ sessionId: String) async throws

    /// Resume failed session with context
    func resumeSession(_ sessionId: String, additionalPrompt: String) async throws

    /// Get all active sessions
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

struct AgentSessionInfo {
    let sessionId: String
    let taskId: TaskID
    let agent: AgentType
    let status: SessionStatus
    let progress: SessionProgress?
}

enum SessionStatus {
    case starting
    case running
    case completing
    case completed
    case failed(Error)
    case cancelled
}

struct SessionProgress {
    let currentStep: String
    let totalToolCalls: Int
    let elapsedTime: TimeInterval
}
```

**Implementation**:
```swift
func delegate(_ task: OvernightTask) async throws -> AgentSessionResult {
    // 1. Get agent provider from registry
    let registry = AgentRegistry.shared
    guard let provider = await registry.provider(for: task.decision.agent) else {
        throw OrchestratorError.agentNotAvailable(task.decision.agent)
    }

    // 2. Start session with task prompt
    let sessionId = UUID().uuidString
    let prompt = task.decision.task

    try await provider.start(
        sessionId: sessionId,
        prompt: prompt,
        workingDirectory: workspaceURL,
        environment: ["OVERNIGHT_MODE": "true"]
    )

    // 3. Subscribe to SessionUpdateHub
    var toolCalls: [ACPToolCallWire] = []
    var lastUpdate: Date = .now

    for await update in SessionUpdateHub.shared.updates(for: sessionId) {
        lastUpdate = .now

        switch update.type {
        case .tool_call(let call):
            toolCalls.append(call)
        case .agent_message_chunk:
            // Log progress
            break
        case .error(let error):
            throw OrchestratorError.agentError(error)
        }

        // Check time budget
        if Date.now.timeIntervalSince(task.startedAt!) > task.decision.estimatedDuration * 1.5 {
            await cancelSession(sessionId)
            throw OrchestratorError.timeBudgetExceeded
        }
    }

    // 4. Return result
    return AgentSessionResult(
        sessionId: sessionId,
        agent: task.decision.agent,
        startedAt: task.startedAt!,
        completedAt: .now,
        success: true,
        toolCalls: toolCalls,
        totalDuration: Date.now.timeIntervalSince(task.startedAt!),
        error: nil
    )
}
```

**Error Handling**:
- If agent crashes: try to resume once with context
- If time budget exceeded: cancel gracefully, mark task as failed
- If AUP violation detected: cancel immediately, log violation

**Dependencies**:
- AgentRegistry (existing)
- AgentProvider (existing)
- SessionUpdateHub (existing)
- PolicyEnforcer

---

### 5. PRAutomationService

**Responsibility**: Create GitHub PRs from agent work

**Location**: `ios/OpenAgentsCore/Sources/OpenAgentsCore/GitHubIntegration/PRAutomationService.swift`

**Interface**:
```swift
actor PRAutomationService {
    /// Create branch for agent session
    func createBranch(baseBranch: String, sessionId: String) async throws -> String

    /// Generate commits from tool calls
    func commitFromToolCalls(_ sessionId: String, toolCalls: [ACPToolCallWire]) async throws

    /// Push branch to remote
    func push(branch: String, remote: String) async throws

    /// Create pull request
    func createPR(
        title: String,
        body: String,
        branch: String,
        baseBranch: String,
        draft: Bool
    ) async throws -> Int  // PR number

    /// Generate PR body from session context
    func generatePRBody(
        task: OvernightTask,
        result: AgentSessionResult
    ) async throws -> String
}
```

**Implementation**:
```swift
func createPR(title: String, body: String, branch: String, baseBranch: String, draft: Bool) async throws -> Int {
    // Use gh CLI (assumes authenticated via system keychain)
    let process = Process()
    process.executableURL = URL(fileURLWithPath: "/opt/homebrew/bin/gh")
    process.arguments = [
        "pr", "create",
        "--title", title,
        "--body", body,
        "--base", baseBranch,
        "--head", branch,
        draft ? "--draft" : ""
    ].filter { !$0.isEmpty }

    let pipe = Pipe()
    process.standardOutput = pipe

    try process.run()
    process.waitUntilExit()

    guard process.terminationStatus == 0 else {
        throw GitHubError.prCreationFailed(process.terminationStatus)
    }

    // Parse PR number from output
    let data = pipe.fileHandleForReading.readDataToEndOfFile()
    let output = String(data: data, encoding: .utf8) ?? ""

    // Extract PR number from URL: https://github.com/owner/repo/pull/123
    let regex = try NSRegularExpression(pattern: #"/pull/(\d+)"#)
    guard let match = regex.firstMatch(in: output, range: NSRange(output.startIndex..., in: output)),
          let range = Range(match.range(at: 1), in: output),
          let prNumber = Int(output[range]) else {
        throw GitHubError.couldNotParsePRNumber
    }

    return prNumber
}

func generatePRBody(task: OvernightTask, result: AgentSessionResult) async throws -> String {
    return """
    ## Autonomous Agent Work

    **Task**: \(task.decision.task)

    **Agent**: \(task.decision.agent.rawValue)

    **Rationale**: \(task.decision.rationale)

    **Session**: `\(result.sessionId)`

    **Duration**: \(formatDuration(result.totalDuration))

    **Tool Calls**: \(result.toolCalls.count)

    ---

    ### Summary

    \(await generateSummaryFromToolCalls(result.toolCalls))

    ---

    ### Files Changed

    \(await listChangedFiles(result.sessionId))

    ---

    *Generated with [OpenAgents Overnight Orchestration](https://github.com/OpenAgentsInc/openagents)*
    """
}
```

**Dependencies**:
- `gh` CLI (installed via Homebrew)
- Git (system)
- Process (Foundation)

---

### 6. UpgradeExecutor

**Responsibility**: Load, validate, and execute upgrade manifests

**Location**: `ios/OpenAgentsCore/Sources/OpenAgentsCore/Upgrades/UpgradeExecutor.swift`

**Interface**:
```swift
actor UpgradeExecutor {
    /// Load manifest from file
    func load(_ manifestPath: URL) async throws -> UpgradeManifest

    /// Validate manifest schema and permissions
    func validate(_ manifest: UpgradeManifest) async throws

    /// Execute pipeline operations
    func execute(_ pipeline: [UpgradeOperation], context: ExecutionContext) async throws -> ExecutionResult

    /// Check if operation is allowed
    func isAllowed(_ op: UpgradeOperation, permissions: UpgradePermissions) -> Bool
}

struct UpgradeManifest: Codable {
    let id: String
    let version: String
    let title: String
    let description: String
    let author: UpgradeAuthor
    let capabilities: UpgradeCapabilities
    let permissions: UpgradePermissions
    let schedule: UpgradeSchedule
    let pipeline: [UpgradeOperation]
    let pricing: UpgradePricing?
    let policy: UpgradePolicy
    let signing: UpgradeSigning?
}

struct UpgradeOperation: Codable {
    let op: String  // "session.analyze", "orchestrate.decide", "agent.execute", etc.
    let params: [String: JSONValue]?
    let backend: String?  // "foundation_models", "mlx", etc.
    let output_var: String?  // Variable name for result
}

struct ExecutionContext {
    var variables: [String: JSONValue]  // Populated from previous ops
    let workingDir: URL
    let timeBudget: TimeInterval
}

struct ExecutionResult {
    let success: Bool
    let outputs: [String: JSONValue]  // All output variables
    let duration: TimeInterval
    let error: String?
}
```

**Operations Registry**:
```swift
enum KnownOperation {
    case sessionAnalyze
    case orchestrateDecide
    case agentExecute
    case prCreate
    case gitClone
    case modelEmbed
    // ... more

    var handler: (UpgradeOperation, ExecutionContext) async throws -> JSONValue {
        switch self {
        case .sessionAnalyze:
            return { op, ctx in
                // Call SessionHistoryAnalyzer
                let insights = try await SessionHistoryAnalyzer.shared.analyze(...)
                return JSONValue.object(insights.toDictionary())
            }
        case .orchestrateDecide:
            return { op, ctx in
                // Call DecisionOrchestrator
                let decision = try await DecisionOrchestrator.shared.decideNextTask(...)
                return JSONValue.object(decision.toDictionary())
            }
        case .agentExecute:
            return { op, ctx in
                // Call AgentCoordinator
                let result = try await AgentCoordinator.shared.delegate(...)
                return JSONValue.object(result.toDictionary())
            }
        // ... more
        }
    }
}
```

**Dependencies**:
- JSONDecoder/Encoder
- Codable validation
- Operations registry
- PolicyEnforcer (permission checks)

---

### 7. PolicyEnforcer

**Responsibility**: Safety, compliance, and resource limits

**Location**: `ios/OpenAgentsCore/Sources/OpenAgentsCore/Orchestration/PolicyEnforcer.swift`

**Interface**:
```swift
actor PolicyEnforcer {
    /// Check Foundation Models AUP compliance
    func checkAUP(_ prompt: String) async throws -> PolicyResult

    /// Validate filesystem permissions
    func checkFileAccess(_ path: String, mode: FileAccessMode, permissions: UpgradePermissions) -> Bool

    /// Check resource limits
    func checkResourceLimits() async throws -> ResourceStatus

    /// Enforce time budget
    func enforceTimeBudget(_ task: OvernightTask, elapsed: TimeInterval) -> TimeBudgetResult
}

enum PolicyResult {
    case allowed
    case denied(reason: String)
    case warning(message: String)
}

struct ResourceStatus {
    let cpuUsage: Double  // 0.0-1.0
    let memoryUsage: UInt64  // bytes
    let diskAvailable: UInt64  // bytes
    let withinLimits: Bool
}

enum TimeBudgetResult {
    case withinBudget
    case approaching(remaining: TimeInterval)
    case exceeded
}
```

**AUP Checks** (DPLA §3.3.8(I)):
```swift
let prohibitedPatterns = [
    "violence", "pornography", "self-harm", "fraud",
    "regulated advice" (healthcare/legal/financial),
    "identify training data", "academic textbooks",
    "circumvent safety"
]

func checkAUP(_ prompt: String) async throws -> PolicyResult {
    for pattern in prohibitedPatterns {
        if prompt.lowercased().contains(pattern) {
            return .denied(reason: "Potential AUP violation: \(pattern)")
        }
    }

    // Future: Use on-device classifier
    return .allowed
}
```

**Dependencies**:
- IOKit (resource usage)
- FileManager (path validation)
- UpgradePermissions

---

## Data Flow

### End-to-End Overnight Run

```
1. SchedulerService.start()
   │
   ├──▶ Parse cron expression
   ├──▶ Calculate next wake time
   └──▶ Start timer loop
        │
        ▼
2. Timer fires (e.g., 1:30 AM)
   │
   ├──▶ Check constraints (plugged_in, wifi, cpu, etc.)
   │    └──▶ If not satisfied: skip, reschedule
   │
   └──▶ If satisfied: continue
        │
        ▼
3. UpgradeExecutor.execute(manifest.pipeline)
   │
   ├──▶ Op 1: session.analyze
   │    └──▶ SessionHistoryAnalyzer reads .jsonl files
   │         └──▶ Returns: sessionInsights
   │
   ├──▶ Op 2: orchestrate.decide (uses sessionInsights)
   │    └──▶ DecisionOrchestrator
   │         ├──▶ Build OrchestrationContext
   │         ├──▶ Call Foundation Models with prompt
   │         ├──▶ Parse JSON response
   │         └──▶ Returns: TaskDecision
   │
   ├──▶ Op 3: agent.execute (uses TaskDecision)
   │    └──▶ AgentCoordinator
   │         ├──▶ Create OvernightTask
   │         ├──▶ TaskQueue.enqueue()
   │         ├──▶ PolicyEnforcer.checkAUP()
   │         ├──▶ TaskQueue.dequeue()
   │         ├──▶ Get AgentProvider from registry
   │         ├──▶ provider.start(sessionId, prompt)
   │         ├──▶ Monitor SessionUpdateHub stream
   │         │    ├──▶ Broadcast to iOS via DesktopWebSocketServer
   │         │    └──▶ Persist to Tinyvex
   │         ├──▶ Wait for completion (or timeout)
   │         └──▶ Returns: AgentSessionResult
   │
   └──▶ Op 4: pr.create (uses AgentSessionResult)
        └──▶ PRAutomationService
             ├──▶ createBranch("agent/session-abc123")
             ├──▶ commitFromToolCalls(result.toolCalls)
             ├──▶ push("origin", "agent/session-abc123")
             ├──▶ generatePRBody(task, result)
             ├──▶ createPR(...) via gh CLI
             └──▶ Returns: PR number
        │
        ▼
4. TaskQueue.updateStatus(taskId, .completed)
   │
   └──▶ Broadcast update to iOS

5. SchedulerService calculates next wake time
   │
   └──▶ Wait until next interval (e.g., 2:00 AM)
        │
        └──▶ Repeat from step 2
```

---

## State Management

### Persistent State (SQLite/Tinyvex)

**Tables**:
1. `overnight_tasks` (TaskQueue)
2. `agent_sessions` (existing Tinyvex schema)
3. `orchestration_decisions` (DecisionOrchestrator cache)
4. `upgrade_manifests` (installed upgrades)
5. `scheduler_state` (last run, next run, status)

**Migrations**:
- Use Tinyvex migration system
- Version 1: Initial schema
- Version 2: Add orchestration tables

### In-Memory State (Actors)

All components are `actor` types:
- Thread-safe by default
- Async access to state
- No race conditions

**Observable State**:
- Use `AsyncStream` for real-time updates
- Bridge to iOS via WebSocket
- Use Combine for SwiftUI bindings

---

## Error Handling

### Error Categories

1. **Recoverable** (retry with backoff):
   - Network timeout
   - Agent process crash (resume with context)
   - GitHub rate limit

2. **User-Actionable** (alert user):
   - AUP violation (show policy, allow override)
   - Resource limit exceeded (ask to increase limits)
   - Git conflict (show diff, ask for resolution)

3. **Fatal** (stop execution):
   - Filesystem permission denied (critical)
   - Invalid upgrade manifest (malformed JSON)
   - FM unavailable on unsupported macOS version

### Retry Logic

```swift
func executeWithRetry<T>(
    maxAttempts: Int = 3,
    backoff: TimeInterval = 2.0,
    operation: () async throws -> T
) async throws -> T {
    var attempt = 0
    var lastError: Error?

    while attempt < maxAttempts {
        do {
            return try await operation()
        } catch let error as RecoverableError {
            lastError = error
            attempt += 1
            try await Task.sleep(nanoseconds: UInt64(backoff * Double(attempt) * 1_000_000_000))
        } catch {
            // Non-recoverable error, throw immediately
            throw error
        }
    }

    throw ExecutionError.maxRetriesExceeded(lastError!)
}
```

---

## Security & Safety

### Sandboxing

- Agents run in user's workspace (no container)
- Filesystem permissions enforced by UpgradePermissions
- Network access whitelisted (allowed_domains)
- Tool usage whitelisted (allowed_tools)

### Code Signing (Future)

- Upgrade manifests signed with Nostr key
- Verify signature before execution
- Revocation list (kind 30055 events)

### AUP Compliance

- Check all prompts before FM invocation
- Log all decisions (audit trail)
- User can review and override

---

## Performance Considerations

### Caching

- Session insights cached for 1 hour (expensive to recompute)
- Repo status cached for 5 minutes (git operations slow)
- FM responses cached per unique prompt hash

### Concurrency

- Run multiple agents in parallel (Claude + Codex concurrently)
- Use `TaskGroup` for parallel pipeline ops
- Limit max concurrent sessions (default: 2)

### Resource Limits

- CPU cap: 80% (respect user's active work)
- Memory cap: 4 GB per agent session
- Disk I/O: throttle writes to 10 MB/s
- Network: respect metered connections

---

## Integration Points

### With Existing Codebase

1. **AgentProvider** (ios/OpenAgentsCore/Sources/OpenAgentsCore/Agents/):
   - Use existing CLIAgentProvider infrastructure
   - ClaudeCodeAgentProvider and CodexAgentProvider unchanged
   - Add new method: `resumeWithContext(sessionId:, context:)`

2. **FMOrchestrator** (ios/OpenAgentsCore/Sources/OpenAgentsCore/Orchestration/FMOrchestrator.swift):
   - Use existing LanguageModelSession wrapper
   - Add new tools: `repo.status`, `repo.coverage`
   - Extend prompt templates for overnight mode

3. **SessionUpdateHub** (existing):
   - No changes required
   - All ACP updates flow through existing hub
   - DecisionOrchestrator publishes custom update types

4. **DesktopWebSocketServer** (existing):
   - Add new JSON-RPC methods:
     - `orchestration/status` → get queue state
     - `orchestration/pause` → pause scheduler
     - `orchestration/resume` → resume scheduler
     - `orchestration/cancel_task` → cancel specific task

5. **Tinyvex** (existing):
   - Add new tables via migration
   - Use existing append-only log pattern
   - Use existing query APIs

### With iOS App

**New SwiftUI Views**:
- `OvernightMonitoringView` (top-level)
- `TaskQueueCard` (pending/active/done counts)
- `SessionCard` (agent progress, tool calls)
- `DecisionCard` (FM rationale, task description)
- `PRPreviewCard` (approve/edit/cancel)

**Bridge Messages** (new JSON-RPC methods):
```json
// macOS → iOS: Task queued
{
  "jsonrpc": "2.0",
  "method": "orchestration/task_queued",
  "params": {
    "task_id": "uuid",
    "decision": { ... }
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

## Next Steps

1. **Implement SchedulerService** (Issue #001)
2. **Implement DecisionOrchestrator** (Issue #002)
3. **Implement TaskQueue** (Issue #003)
4. **Implement AgentCoordinator** (Issue #004)
5. **Implement PRAutomationService** (Issue #005)
6. **Implement UpgradeExecutor** (Issue #006)
7. **Implement PolicyEnforcer** (Issue #007)
8. **iOS Bridge Integration** (Issue #008)
9. **Create Example Upgrades** (Issue #009)
10. **End-to-End Testing** (Issue #010)
11. **Demo Preparation** (Issue #011)
12. **Documentation & ADR** (Issue #012)

See `issues/` directory for detailed implementation specs.
