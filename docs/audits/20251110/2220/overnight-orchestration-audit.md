# Overnight Orchestration Implementation Audit

**Date**: 2025-11-10
**Time**: 22:20
**Auditor**: Claude Code (Sonnet 4.5)
**Scope**: Overnight orchestration plan implementation status and path to overnight coding capability

---

## Executive Summary

The OpenAgents codebase has **substantial orchestration infrastructure already implemented**, representing ~70-80% of the overnight vision outlined in `docs/overnight/`. The core components (SchedulerService, TaskQueue, AgentCoordinator, DecisionEngine, OrchestrationConfig) exist and are well-tested. The current delegation flow in the macOS app successfully delegates from Foundation Models to Codex/Claude Code with full ACP streaming and concurrent delegation support.

**Key Finding**: The architecture is production-ready for overnight orchestration. The missing pieces are primarily:
1. System-level constraint enforcement (battery API, network monitoring)
2. GitHub/PR automation service integration
3. Process management for overnight keep-alive
4. Upgrade manifest execution framework (optional - can defer)

**Status**: **~75% complete** for minimal viable overnight orchestration. The remaining 25% involves system integration work (IOKit, NetworkKit APIs) and GitHub automation, not architectural changes.

---

## Current Delegation Flow (macOS App)

### How It Works Today

The macOS app uses **Foundation Models as the default orchestrator** with a `delegate.run` tool for task delegation:

```
User Prompt → OpenAgents (Foundation Models)
              ↓
         [FM analyzes and decides]
              ↓
         Calls FMTool_DelegateRun
              ↓
    Creates sub-session (unique ID)
              ↓
    Maps sub-session → parent session
              ↓
    Delegates to Codex or Claude Code
              ↓
    Streams ACP updates to parent
              ↓
    Supports concurrent delegations
```

**Key Files**:
- `ios/OpenAgentsCore/Sources/OpenAgentsCore/Agents/OpenAgentsLocalProvider.swift` - FM orchestrator with delegate tool
- `ios/OpenAgentsCore/Sources/OpenAgentsCore/Agents/CodexAgentProvider.swift` - Codex provider
- `ios/OpenAgentsCore/Sources/OpenAgentsCore/Agents/CLIAgentProvider.swift` - Base CLI agent class
- `ios/OpenAgentsCore/Sources/OpenAgentsCore/DesktopBridge/SessionUpdateHub.swift` - Update streaming

**Recent Enhancements** (Per Git History):
- #1465: Wire user reply routing for conversational orchestration setup (CLOSED 2025-11-09)
- #1464: Conversational orchestration setup via orchestrator agent (CLOSED 2025-11-09)
- #1462: Orchestration aiming UI for goals/schedule/preferences (CLOSED 2025-11-09)
- Concurrent delegation support with comprehensive tests (commit 03a0afd3)

**Strengths**:
- ✅ Full ACP compliance for all agent communication
- ✅ Session mapping forwards updates properly
- ✅ Concurrent delegations work (3+ simultaneous)
- ✅ Chunk interleaving handled correctly
- ✅ Thread ID management and resume capability (Codex)
- ✅ Conversational routing via ConversationalDetection

---

## Implementation Status by Component

### ✅ FULLY IMPLEMENTED

#### 1. TaskQueue
**File**: `ios/OpenAgentsCore/Sources/OpenAgentsCore/Orchestration/TaskQueue.swift`

- ✅ SQLite-backed persistence via Tinyvex
- ✅ Task lifecycle: pending → in_progress → completed/failed/cancelled
- ✅ Deduplication via operation hash
- ✅ FIFO queue with priority support
- ✅ Validated status transitions
- ✅ Comprehensive tests (`TaskQueueTests.swift`)

**Status**: Production-ready, no changes needed.

#### 2. DecisionEngine
**File**: `ios/OpenAgentsCore/Sources/OpenAgentsCore/Orchestration/DecisionEngine.swift`

- ✅ SessionAnalyzeTool integration for history analysis
- ✅ Heuristic decision logic (refactor vs test paths)
- ✅ Config-aware decisions (goals bias decision path)
- ✅ Agent preference filtering (prefer/allow lists)
- ✅ Confidence scoring and rationale generation
- ✅ Comprehensive tests (`DecisionEngineTests.swift`)

**Current Decision Logic**:
```swift
// Refactor path: top file touched >20x + "refactor" intent → Claude Code
// Tests path: no refactor signal → Codex
// Fallback: exploratory analysis
```

**Status**: Production-ready, extensible for FM-based decisions.

#### 3. AgentCoordinator
**File**: `ios/OpenAgentsCore/Sources/OpenAgentsCore/Orchestration/AgentCoordinator.swift`

- ✅ Core orchestration loop (`runCycle()`)
- ✅ Task queue integration
- ✅ DecisionEngine integration
- ✅ AgentRegistry integration
- ✅ Timeout enforcement for long-running tasks
- ✅ Metrics tracking (cycles, tasks executed/completed/failed)
- ✅ Session monitoring via SessionUpdateHub
- ✅ Comprehensive tests (`AgentCoordinatorTests.swift`, `CoordinatorLocalTests.swift`)

**Status**: Production-ready, core loop working.

#### 4. OrchestrationConfig
**File**: `ios/OpenAgentsCore/Sources/OpenAgentsCore/Orchestration/OrchestrationConfig.swift`

- ✅ Complete configuration model with validation
- ✅ Schedule (cron expression, time window, jitter, on_missed policy)
- ✅ Constraints (pluggedIn, wifiOnly - **definitions only, not enforced**)
- ✅ Time budget (15 min - 2 hours)
- ✅ Max concurrent tasks (1-4)
- ✅ User goals (bias decision logic)
- ✅ Agent preferences (prefer/allow filtering)
- ✅ File focus (include/exclude globs)
- ✅ PR automation settings
- ✅ SQLite persistence via Tinyvex
- ✅ Comprehensive tests (`OrchestrationConfigTests.swift`)

**Status**: Production-ready, constraint enforcement needs implementation.

#### 5. SchedulerService
**File**: `ios/OpenAgentsCore/Sources/OpenAgentsCore/Orchestration/SchedulerService.swift`

- ✅ Timer-based orchestration with cron scheduling
- ✅ Computes next wake time via SchedulePreview
- ✅ Simple start/stop/status interface
- ✅ Integrates with AgentCoordinator for cycle execution
- ✅ Minimal, testable implementation

**Current Implementation**:
```swift
public actor SchedulerService {
    public enum State: Equatable {
        case idle, running(nextWake: Date?), stopped
    }

    public func configure(config: OrchestrationConfig, trigger: @escaping () async -> Void)
    public func start()
    public func stop()
    public func status() -> State

    public static func nextWake(for schedule: Schedule, from: Date) -> Date?
}
```

**What's Implemented**:
- Basic timer loop using Task.sleep
- Cron expression support via SchedulePreview
- Trigger callback for orchestration cycles
- State tracking (idle/running/stopped)

**What's Missing**:
- Constraint checking (pluggedIn, wifiOnly, CPU, DND) - **always runs**
- Time window enforcement - **computed but not checked**
- Jitter application - **computed but not applied**
- Catch-up policy - **not implemented**
- Sleep/wake coordination - **no system integration**

**Status**: Basic scaffold exists, needs constraint enforcement.

#### 6. Bridge Integration
**File**: `ios/OpenAgentsCore/Sources/OpenAgentsCore/DesktopBridge/DesktopWebSocketServer+Orchestration.swift`

- ✅ Full RPC support for orchestration:
  - `orchestrate.explore.start` - FM-based exploration
  - `orchestrate.config.get/set/list/activate` - Config management
  - `orchestrate.scheduler.reload/status/run_now` - Scheduler control
  - `orchestrate.coordinator.run_once/status` - Manual triggers
  - `orchestrate.setup.start/status/abort` - Conversational setup
- ✅ Active config caching
- ✅ Scheduler binding to configs
- ✅ Capability gating (ACPExt)
- ✅ Comprehensive tests (`OrchestrateRPCTests.swift`)

**Status**: Production-ready, full bridge coverage.

#### 7. Supporting Infrastructure

**ExploreOrchestrator** (`ExploreOrchestrator.swift`):
- ✅ FM-based exploration with tool streaming
- ✅ Session tools integration (SessionAnalyzeTool, SessionListTool, etc.)
- ✅ State machine reducers
- ✅ ACP tool call streaming
- ✅ Tests (`ExploreOrchestratorTests.swift` - deleted, replaced with integration tests)

**SessionTools** (`SessionTools.swift`):
- ✅ SessionAnalyzeTool - File frequency, tool patterns, user intent
- ✅ SessionListTool - Recent sessions
- ✅ SessionSearchTool - Search by keywords
- ✅ SessionReadTool - Read session details

**ConversationalDetection** (`Routing/ConversationalDetection.swift`):
- ✅ Intelligent routing based on prompt content
- ✅ Detects conversational vs coding prompts
- ✅ Auto-routes to orchestrator or specialized agents
- ✅ Tests (`ConversationalDetectionTests.swift`)

### ⚠️ PARTIALLY IMPLEMENTED

#### 8. Constraint Checking

**Current State**: Configuration fields exist, but enforcement is **not implemented**.

**What Exists**:
```swift
// In OrchestrationConfig.swift
public struct Constraints: Codable, Sendable, Equatable {
    public var pluggedIn: Bool
    public var wifiOnly: Bool
    // ... other fields
}
```

**What's Missing**:
- ❌ Battery/power status monitoring (IOKit API)
- ❌ Network type monitoring (NWPathMonitor)
- ❌ CPU usage monitoring (`host_processor_info` deltas)
- ❌ DND/focus mode detection
- ❌ User activity detection (NSWorkspace)

**Recommended Implementation** (from `docs/overnight/issues/001-scheduler-service.md`):
```swift
protocol ConstraintChecker {
    func check() async -> Bool
}

actor PluggedInChecker: ConstraintChecker {
    func check() async -> Bool {
        let snapshot = IOPSCopyPowerSourcesInfo().takeRetainedValue()
        let sources = IOPSCopyPowerSourcesList(snapshot).takeRetainedValue() as Array
        for source in sources {
            if let description = IOPSGetPowerSourceDescription(snapshot, source)
                .takeUnretainedValue() as? [String: Any],
               let powerSourceState = description[kIOPSPowerSourceStateKey] as? String {
                return powerSourceState == kIOPSACPowerValue
            }
        }
        return false
    }
}

actor WiFiOnlyChecker: ConstraintChecker {
    func check() async -> Bool {
        return await withCheckedContinuation { continuation in
            let monitor = NWPathMonitor()
            monitor.pathUpdateHandler = { path in
                let isWiFi = path.usesInterfaceType(.wifi)
                monitor.cancel()
                continuation.resume(returning: isWiFi)
            }
            monitor.start(queue: DispatchQueue.global())
        }
    }
}
```

**Priority**: P0 for overnight coding (must not drain battery or use cellular).

### ❌ NOT IMPLEMENTED

#### 9. PRAutomationService

**Planned File**: `ios/OpenAgentsCore/Sources/OpenAgentsCore/GitHubIntegration/PRAutomationService.swift`

**What's Needed** (from `docs/overnight/README.md`):
```swift
actor PRAutomationService {
    func createBranch(from baseBranch: String, name: String) async throws
    func commitFromToolCalls(_ toolCalls: [ACPToolCallWire]) async throws
    func push(branch: String, remote: String) async throws
    func createPR(title: String, body: String, branch: String) async throws -> PRNumber
}
```

**Current Workaround**: None. No PR automation exists.

**Implementation Notes**:
- Use `gh` CLI for PR creation (already used for issue management)
- Discover `gh` via PATH using CLIAgentProvider's `findBinary()` strategy
- Generate commit messages from ACP tool call context
- Create branch names like `agent/{session_id}` or `overnight/{timestamp}`
- Include orchestration metadata in PR body

**Challenges**:
- Deriving file changes from ACP tool calls is brittle
- Current providers don't expose clean file change lists
- May need to commit entire working tree and generate summary from DecisionOutput

**Priority**: P0 for overnight demo (core value prop is waking up to PRs).

#### 10. UpgradeExecutor / Manifest Runtime

**Planned File**: `ios/OpenAgentsCore/Sources/OpenAgentsCore/Upgrades/UpgradeExecutor.swift`

**Vision** (from `docs/overnight/README.md`):
- Parse JSON upgrade manifests
- Execute declarative pipelines (ops registry)
- Support operations: `session.analyze`, `orchestrate.decide`, `agent.execute`, `pr.create`
- Integrate with SchedulerService
- Validation and safety checks
- Nostr signing/publishing for marketplace

**Current State**: No implementation. Manifests exist as documentation only.

**Recommendation**: **Defer for post-demo**. The current approach (hardcoded orchestration loop) is sufficient for proving the concept. Build the manifest runtime after validating product-market fit.

**Alternative Minimal Path**:
- Create `OvernightRunner` that reads a simple JSON config (schedule + goals)
- Hardcode the pipeline: analyze → decide → execute → PR
- Skip the full operations registry and variable interpolation
- Focus on proving overnight value, not the marketplace vision

**Priority**: P2 (nice-to-have for demo, required for marketplace).

#### 11. PolicyEnforcer

**Planned File**: `ios/OpenAgentsCore/Sources/OpenAgentsCore/Orchestration/PolicyEnforcer.swift`

**Purpose**:
- AUP compliance checks (block dangerous operations)
- Resource limits (CPU, memory, disk)
- Workspace permissions validation
- Time budget enforcement

**Current State**: No implementation. Time budget enforcement exists in AgentCoordinator.

**Recommendation**: **Defer for post-demo**. Add basic AUP keyword filtering if needed, but don't block on this.

**Priority**: P2 (safety feature, not critical for demo).

---

## Gap Analysis: What's Missing for Overnight Coding

### Critical Gaps (P0) - Must Have

1. **System Constraint Enforcement**
   - Battery/power status monitoring (IOKit)
   - Network type monitoring (NWPathMonitor)
   - CPU usage monitoring (optional for demo)
   - Integration into SchedulerService.start() loop
   - **Estimated Effort**: 1-2 days

2. **PR Automation Service**
   - GitHub integration via `gh` CLI
   - Branch creation and management
   - Commit generation from tool calls or working tree
   - PR creation with orchestration metadata
   - **Estimated Effort**: 2-3 days

3. **Process Keep-Alive for Overnight Runs**
   - Prevent macOS from suspending the app
   - Handle sleep/wake cycles gracefully
   - Timer continuation after wake
   - **Estimated Effort**: 1 day

4. **End-to-End Integration Testing**
   - Compressed overnight run (5-10 min test)
   - Multi-cycle orchestration flow
   - PR creation validation
   - **Estimated Effort**: 1 day

**Total P0 Effort**: 5-7 days

### Important Gaps (P1) - Should Have

5. **Time Window Enforcement**
   - Check if current time is within schedule.window
   - Skip execution if outside window
   - **Estimated Effort**: 0.5 days (simple)

6. **Jitter Application**
   - Apply random delay after scheduled time
   - Prevent thundering herd
   - **Estimated Effort**: 0.5 days (simple)

7. **Catch-Up Policy**
   - Implement `skip` vs `run_once_at_next_opportunity`
   - State persistence for missed runs
   - **Estimated Effort**: 1 day

8. **Completion Notifications**
   - System notification when overnight run completes
   - Summary of PRs created
   - **Estimated Effort**: 0.5 days

**Total P1 Effort**: 2.5 days

### Nice-to-Have Gaps (P2) - Can Defer

9. **Upgrade Manifest Runtime**
   - Full UpgradeExecutor implementation
   - Operations registry
   - Variable interpolation
   - Nostr signing/publishing
   - **Estimated Effort**: 5-7 days

10. **PolicyEnforcer**
    - AUP compliance checks
    - Resource limits monitoring
    - **Estimated Effort**: 2-3 days

11. **Advanced Constraint Checking**
    - DND/focus mode detection
    - User activity detection
    - Advanced CPU monitoring with deltas
    - **Estimated Effort**: 2 days

12. **iOS Monitoring UI**
    - Real-time task queue visualization
    - Agent session status cards
    - Decision rationale display
    - Manual intervention controls
    - **Estimated Effort**: 3-4 days

**Total P2 Effort**: 12-16 days

---

## Current vs. Planned Architecture

### Current State (What Exists Today)

```
┌─────────────────────────────────────────────────────────────┐
│                      macOS Desktop                           │
├─────────────────────────────────────────────────────────────┤
│                                                               │
│  ┌─────────────────┐         ┌──────────────────┐           │
│  │ SchedulerService│────────▶│ AgentCoordinator │           │
│  │ (Timer: basic)  │         │ (runCycle)       │           │
│  └─────────────────┘         └──────────────────┘           │
│          │                            │                      │
│          ▼                            ▼                      │
│  ┌─────────────────────────────────────────────┐            │
│  │     DecisionEngine                          │            │
│  │  ┌──────────────────────────────────────┐   │            │
│  │  │  Heuristic Logic                     │   │            │
│  │  │  + SessionAnalyzeTool                │   │            │
│  │  │  + Config-aware decisions            │   │            │
│  │  └──────────────────────────────────────┘   │            │
│  └─────────────────────────────────────────────┘            │
│          │                                                   │
│          ▼                                                   │
│  ┌─────────────────┐                                        │
│  │   TaskQueue     │ (SQLite/Tinyvex)                       │
│  │  - pending      │                                        │
│  │  - in_progress  │                                        │
│  │  - completed    │                                        │
│  └─────────────────┘                                        │
│          │                                                   │
│          ▼                                                   │
│  ┌─────────────────────────────────────────────┐            │
│  │      AgentRegistry                          │            │
│  │  ┌──────────────┐    ┌──────────────┐      │            │
│  │  │ Claude Code  │    │ OpenAI Codex │      │            │
│  │  │ Provider     │    │ Provider     │      │            │
│  │  └──────────────┘    └──────────────┘      │            │
│  └─────────────────────────────────────────────┘            │
│          │                                                   │
│          ▼ (ACP SessionUpdate stream)                       │
│  ┌─────────────────────────────────────────────┐            │
│  │    SessionUpdateHub                         │            │
│  │    - Broadcast to bridge clients            │            │
│  │    - Session mapping for delegation         │            │
│  └─────────────────────────────────────────────┘            │
│                                                               │
│  ❌ NO PR AUTOMATION (gap)                                   │
│  ⚠️  NO CONSTRAINT ENFORCEMENT (gap)                         │
│                                                               │
└───────────────────────────────────────────────────────────────┘
           │
           │ (WebSocket Bridge - exists)
           ▼
┌───────────────────────────────────────────────────────────────┐
│                        iOS Device                             │
├───────────────────────────────────────────────────────────────┤
│  ┌─────────────────────────────────────────────┐              │
│  │   Current UI: Chat + Delegation Cards       │              │
│  │   ❌ NO Overnight Monitoring UI (gap)        │              │
│  └─────────────────────────────────────────────┘              │
└───────────────────────────────────────────────────────────────┘
```

### Planned State (Overnight Vision)

```
┌─────────────────────────────────────────────────────────────┐
│                      macOS Desktop                           │
├─────────────────────────────────────────────────────────────┤
│                                                               │
│  ┌─────────────────┐         ┌──────────────────┐           │
│  │ SchedulerService│────────▶│ AgentCoordinator │           │
│  │ + Constraints ✨│         │ (runCycle)       │           │
│  │ + Time Window ✨│         └──────────────────┘           │
│  │ + Jitter ✨     │                  │                      │
│  └─────────────────┘                  │                      │
│          │                            ▼                      │
│          │                    ┌──────────────────┐           │
│          │                    │ DecisionEngine   │           │
│          │                    │ + FM Analysis ✨ │           │
│          │                    └──────────────────┘           │
│          ▼                            │                      │
│  ┌─────────────────────────────────────────────┐            │
│  │     TaskQueue (exists)                      │            │
│  └─────────────────────────────────────────────┘            │
│          │                                                   │
│          ▼                                                   │
│  ┌─────────────────────────────────────────────┐            │
│  │      AgentRegistry (exists)                 │            │
│  └─────────────────────────────────────────────┘            │
│          │                                                   │
│          ▼ (ACP SessionUpdate stream)                       │
│  ┌─────────────────────────────────────────────┐            │
│  │    PRAutomationService ✨                    │            │
│  │    - Create branches                        │            │
│  │    - Generate commits                       │            │
│  │    - Push to GitHub                         │            │
│  │    - Create PRs via gh CLI                  │            │
│  └─────────────────────────────────────────────┘            │
│                                                               │
└───────────────────────────────────────────────────────────────┘
           │
           │ (WebSocket Bridge)
           ▼
┌───────────────────────────────────────────────────────────────┐
│                        iOS Device                             │
├───────────────────────────────────────────────────────────────┤
│  ┌─────────────────────────────────────────────┐              │
│  │   Overnight Monitoring UI ✨                 │              │
│  │  - Real-time task queue                     │              │
│  │  - Agent session cards                      │              │
│  │  - FM decision rationale                    │              │
│  │  - PR preview & approval                    │              │
│  └─────────────────────────────────────────────┘              │
└───────────────────────────────────────────────────────────────┘
```

**Legend**: ✨ = Not implemented or incomplete

---

## Recommended Next Steps

### Phase 1: Minimal Viable Overnight (1-2 weeks)

**Goal**: Ship overnight orchestration with Codex delegation and PR creation.

**Scope**:
1. ✅ Use existing SchedulerService, TaskQueue, AgentCoordinator, DecisionEngine
2. ✨ Implement constraint checking (battery, WiFi) in SchedulerService
3. ✨ Implement PRAutomationService (gh CLI integration)
4. ✨ Add process keep-alive for overnight runs
5. ✨ Add time window and jitter enforcement
6. ✨ Write end-to-end integration test (compressed timeline)
7. ✨ Create demo config JSON (nightly-refactor.json)

**Deliverable**: macOS app runs overnight orchestration, delegates to Codex, creates PRs.

**Success Metrics**:
- SchedulerService wakes every 30 min (1am-5am)
- Checks constraints (plugged in, WiFi) before running
- DecisionEngine makes 5-10 decisions based on session history
- AgentCoordinator delegates to Codex successfully
- PRAutomationService creates 3-5 PRs with quality work
- All tests pass

**Timeline**: 7-10 days (P0 work only)

### Phase 2: Polish & iOS Monitoring (1 week)

**Goal**: Add iOS monitoring UI and polish rough edges.

**Scope**:
1. ✨ iOS orchestration monitoring view
2. ✨ Real-time task queue updates via bridge
3. ✨ Decision rationale display
4. ✨ Manual intervention controls (pause/resume/cancel)
5. ✨ PR preview before push
6. ✨ Completion notifications

**Deliverable**: Full iOS monitoring experience.

**Timeline**: 5-7 days

### Phase 3: Upgrade Manifests (Optional, 1-2 weeks)

**Goal**: Build manifest runtime for marketplace vision.

**Scope**:
1. ✨ JSON manifest parser and validator
2. ✨ Operations registry (`session.analyze`, `orchestrate.decide`, `agent.execute`, `pr.create`)
3. ✨ Pipeline executor with variable interpolation
4. ✨ Nostr signing/publishing hooks
5. ✨ Example manifests (nightly-refactor, feature-worker, test-generator)

**Deliverable**: Declarative orchestration via JSON manifests.

**Timeline**: 10-14 days

**Recommendation**: **Defer to post-demo**. Prove overnight value first.

### Phase 4: Marketplace Integration (Future)

**Scope**:
1. ✨ Nostr marketplace events (kind 30051)
2. ✨ Discover upgrades from relays
3. ✨ Payment coordination via Spark SDK
4. ✨ Reputation system (kind 30054 events)

**Timeline**: 2-3 weeks

**Dependency**: Phase 3 complete.

---

## Technical Deep Dive: How to Extend Current Delegation for Overnight

### Current Delegation Flow (Today)

```swift
// In OpenAgentsLocalProvider.swift
let delegateTool = FMTool_DelegateRun(
    sessionId: parentSessionId,
    registry: agentRegistry,
    updateHub: updateHub,
    workspace: workspace
)

// User prompt → FM → delegate.run(prompt: "...", agent: "codex")
// FM decides to delegate and calls the tool
// Tool creates sub-session and streams updates to parent
```

**Strengths**:
- FM makes intelligent delegation decisions
- Concurrent delegations work
- Full ACP streaming
- Session mapping is clean

**For Overnight**:
- This flow already works for one-off delegations
- Need to wrap it in a periodic scheduler
- Add constraint checking before triggering
- Add PR creation after completion

### Overnight Extension Architecture

```swift
// 1. SchedulerService triggers every 30 min
scheduler.configure(config: orchestrationConfig) { [weak self] in
    await self?.runOvernightCycle()
}

// 2. Check constraints before running
func runOvernightCycle() async {
    // NEW: Check battery, WiFi, time window
    guard await constraintsSatisfied() else {
        logger.info("Constraints not satisfied, skipping cycle")
        return
    }

    // Existing: Run coordinator cycle
    let result = await agentCoordinator.runCycle()

    // NEW: If task completed, create PR
    if case .taskExecuted(let taskId, let sessionId) = result {
        await createPRForSession(taskId: taskId, sessionId: sessionId)
    }
}

// 3. PR creation after task completes
func createPRForSession(taskId: TaskID, sessionId: String) async {
    let prService = PRAutomationService()

    // Get task decision for PR metadata
    let task = try await taskQueue.getTask(taskId)

    // Create branch
    let branchName = "agent/\(sessionId)"
    try await prService.createBranch(from: "main", name: branchName)

    // Commit working tree (simple approach)
    try await prService.commitWorkingTree(message: task.decision.task)

    // Push to remote
    try await prService.push(branch: branchName, remote: "origin")

    // Create PR
    let prNumber = try await prService.createPR(
        title: task.decision.task,
        body: generatePRBody(task: task),
        branch: branchName
    )

    // Update task metadata
    try await taskQueue.updateMetadata(
        taskId: taskId,
        metadata: ["pr_number": "\(prNumber)", "branch": branchName]
    )
}
```

### Constraint Checking Integration

```swift
// Add to SchedulerService
private func constraintsSatisfied() async -> Bool {
    guard let config = self.config else { return false }

    let constraints = config.constraints

    // Check battery
    if constraints.pluggedIn {
        let checker = PluggedInChecker()
        guard await checker.check() else {
            logger.info("Not plugged in, skipping")
            return false
        }
    }

    // Check WiFi
    if constraints.wifiOnly {
        let checker = WiFiOnlyChecker()
        guard await checker.check() else {
            logger.info("Not on WiFi, skipping")
            return false
        }
    }

    // Check time window
    if let window = config.schedule.window {
        let now = Date()
        guard isWithinWindow(now, window: window) else {
            logger.info("Outside time window, skipping")
            return false
        }
    }

    return true
}

private func isWithinWindow(_ time: Date, window: TimeWindow) -> Bool {
    let calendar = Calendar.current
    let components = calendar.dateComponents([.hour, .minute], from: time)
    guard let hour = components.hour, let minute = components.minute else {
        return false
    }

    let currentMinutes = hour * 60 + minute

    // Parse start/end (format: "HH:mm")
    let startComponents = window.start.split(separator: ":").compactMap { Int($0) }
    let endComponents = window.end.split(separator: ":").compactMap { Int($0) }

    guard startComponents.count == 2, endComponents.count == 2 else {
        return false
    }

    let startMinutes = startComponents[0] * 60 + startComponents[1]
    let endMinutes = endComponents[0] * 60 + endComponents[1]

    if startMinutes <= endMinutes {
        // Normal range (e.g., 01:00 - 05:00)
        return currentMinutes >= startMinutes && currentMinutes <= endMinutes
    } else {
        // Crosses midnight (e.g., 22:00 - 02:00)
        return currentMinutes >= startMinutes || currentMinutes <= endMinutes
    }
}
```

### Process Keep-Alive

```swift
// In SchedulerService or BridgeManager
#if os(macOS)
import IOKit.pwr_mgt

private var assertionID: IOPMAssertionID = 0

func preventSleep() {
    var assertionID: IOPMAssertionID = 0
    let result = IOPMAssertionCreateWithName(
        kIOPMAssertionTypePreventUserIdleSystemSleep as CFString,
        IOPMAssertionLevel(kIOPMAssertionLevelOn),
        "OpenAgents Overnight Orchestration" as CFString,
        &self.assertionID
    )

    if result == kIOReturnSuccess {
        logger.info("Sleep prevention enabled")
    } else {
        logger.error("Failed to prevent sleep: \(result)")
    }
}

func allowSleep() {
    if assertionID != 0 {
        IOPMAssertionRelease(assertionID)
        assertionID = 0
        logger.info("Sleep prevention disabled")
    }
}
#endif
```

---

## Files to Create/Modify

### New Files (Required)

1. **`ios/OpenAgentsCore/Sources/OpenAgentsCore/Orchestration/ConstraintCheckers.swift`**
   - PluggedInChecker (IOKit)
   - WiFiOnlyChecker (NWPathMonitor)
   - CPUChecker (optional for demo)
   - Protocol and implementations

2. **`ios/OpenAgentsCore/Sources/OpenAgentsCore/GitHubIntegration/PRAutomationService.swift`**
   - Branch management
   - Commit generation
   - PR creation via gh CLI
   - Integration with gh discovery

3. **`ios/OpenAgentsCore/Tests/OpenAgentsCoreTests/Orchestration/ConstraintCheckersTests.swift`**
   - Mock IOKit/NetworkKit APIs
   - Test constraint checking logic

4. **`ios/OpenAgentsCore/Tests/OpenAgentsCoreTests/Integration/OvernightOrchestrationIntegrationTests.swift`**
   - End-to-end overnight run (compressed)
   - Multi-cycle orchestration
   - PR creation validation

### Files to Modify

1. **`ios/OpenAgentsCore/Sources/OpenAgentsCore/Orchestration/SchedulerService.swift`**
   - Add constraint checking before trigger
   - Add time window enforcement
   - Add jitter application
   - Add process keep-alive hooks

2. **`ios/OpenAgentsCore/Sources/OpenAgentsCore/Orchestration/AgentCoordinator.swift`**
   - Add PR automation integration after task completion
   - Add completion callbacks

3. **`ios/OpenAgentsCore/Sources/OpenAgentsCore/DesktopBridge/DesktopWebSocketServer+Orchestration.swift`**
   - Add PR metadata to RPC responses
   - Add constraint status reporting

4. **`docs/overnight/architecture.md`** (expand stub)
   - Document constraint checking architecture
   - Document PR automation flow
   - Add sequence diagrams

5. **`docs/overnight/testing-plan.md`** (create)
   - Unit test plan
   - Integration test plan
   - Manual testing scenarios

---

## Open Questions & Decisions Needed

### 1. PR Creation Strategy

**Options**:
- **A. Commit entire working tree** (simple, may include unrelated changes)
- **B. Parse ACP tool calls for file changes** (accurate, but brittle - current providers don't expose clean file lists)
- **C. Track file changes via FileSystemWatcher** (complex, requires new infrastructure)

**Recommendation**: **Option A for demo**, Option B for production (requires provider enhancements).

### 2. Upgrade Manifest Runtime

**Options**:
- **A. Build full UpgradeExecutor now** (aligns with marketplace vision, 2+ weeks)
- **B. Defer to post-demo** (ship faster, prove value first)
- **C. Build minimal OvernightRunner** (hardcoded pipeline, 2-3 days)

**Recommendation**: **Option B** (defer). Focus on proving overnight value before building marketplace infrastructure.

### 3. Foundation Models Decision Logic

**Current**: Heuristic-based (file frequency + keywords)
**Future**: FM-based (analyze sessions with Foundation Models)

**Question**: Use FM for decisions in Phase 1 or stick with heuristics?

**Recommendation**: **Heuristics for Phase 1**, FM integration in Phase 2. Heuristics are deterministic and testable.

### 4. Constraint Enforcement Strictness

**Options**:
- **A. Hard fail** (skip cycle if constraints not met)
- **B. Soft fail with retry** (wait and retry with backoff)
- **C. User-configurable** (config field: `constraint_mode: strict|opportunistic`)

**Recommendation**: **Option A for demo** (simple, predictable). Option C for production.

### 5. iOS Monitoring UI Priority

**Question**: Build iOS monitoring UI in Phase 1 or Phase 2?

**Trade-off**:
- Phase 1: Full demo experience, but delays ship date
- Phase 2: Ship faster, but less polished demo

**Recommendation**: **Phase 2** (defer). macOS-only demo is sufficient for proving overnight value.

---

## Risk Assessment

### High Risk (Likely to Block)

1. **PR automation complexity** - Deriving file changes from sessions is non-trivial
   - **Mitigation**: Start with commit-entire-tree approach for demo

2. **Constraint checking on different macOS versions** - IOKit/NetworkKit APIs may vary
   - **Mitigation**: Test on macOS 13.0, 14.0, 15.0+ (Sequoia)

3. **Process keep-alive reliability** - macOS may still suspend app despite assertions
   - **Mitigation**: Test overnight runs on real hardware, not VM

### Medium Risk (May Cause Delays)

4. **Decision quality** - Heuristic logic may make poor task choices
   - **Mitigation**: Start with simple refactor/test decision logic, expand later

5. **GitHub API rate limits** - gh CLI may hit rate limits with many PRs
   - **Mitigation**: Use GitHub token with higher limits, add rate limit handling

6. **Test reliability** - Integration tests with gh CLI may be flaky
   - **Mitigation**: Mock GitHub API for tests, use skip guards for gh-dependent tests

### Low Risk (Unlikely to Impact)

7. **Database migration** - Adding new tables to Tinyvex
   - **Mitigation**: Tinyvex supports incremental schema evolution

8. **Bridge protocol changes** - New RPC methods for orchestration
   - **Mitigation**: ACPExt already supports capability negotiation

---

## Success Metrics

### Phase 1 Success Criteria

- [ ] SchedulerService runs overnight (8+ hours) without crashing
- [ ] Constraint checking prevents execution when not plugged in / not on WiFi
- [ ] DecisionEngine makes 15-20 decisions based on session history
- [ ] AgentCoordinator delegates to Codex successfully (no hangs or timeouts)
- [ ] PRAutomationService creates 5-10 PRs with quality work
- [ ] All unit tests pass (≥90% coverage on new code)
- [ ] Integration test passes (compressed 10-min overnight run)
- [ ] macOS app remains responsive during overnight runs
- [ ] Battery drain <5% per hour when plugged in

### Demo Video Requirements

1. Show orchestration setup (config creation)
2. Show scheduler start (evening, 6pm)
3. Show time-lapse of overnight run (1am-5am compressed)
4. Show morning GitHub view (8+ PRs created)
5. Show PR quality (code diffs, commit messages, rationale)
6. Show iOS monitoring (optional for Phase 1)
7. Explain upgrade manifest vision and Nostr marketplace

---

## References

### Documentation

- **Overnight Plan**: `docs/overnight/plan.md` - High-level vision and timeline
- **Overnight README**: `docs/overnight/README.md` - Architecture overview and demo flow
- **Overnight Architecture**: `docs/overnight/architecture.md` - Detailed technical design (stub)
- **Previous Audit**: `docs/overnight/issues/999-audit-review-2025-11-08.md` - Comprehensive audit and recommendations
- **Issue Templates**: `docs/overnight/issues/001-scheduler-service.md` through `012-documentation-adr.md`

### Implementation Files

- **SchedulerService**: `ios/OpenAgentsCore/Sources/OpenAgentsCore/Orchestration/SchedulerService.swift:1`
- **TaskQueue**: `ios/OpenAgentsCore/Sources/OpenAgentsCore/Orchestration/TaskQueue.swift:1`
- **AgentCoordinator**: `ios/OpenAgentsCore/Sources/OpenAgentsCore/Orchestration/AgentCoordinator.swift:1`
- **DecisionEngine**: `ios/OpenAgentsCore/Sources/OpenAgentsCore/Orchestration/DecisionEngine.swift:1`
- **OrchestrationConfig**: `ios/OpenAgentsCore/Sources/OpenAgentsCore/Orchestration/OrchestrationConfig.swift:1`
- **Bridge Integration**: `ios/OpenAgentsCore/Sources/OpenAgentsCore/DesktopBridge/DesktopWebSocketServer+Orchestration.swift:1`
- **Delegation**: `ios/OpenAgentsCore/Sources/OpenAgentsCore/Agents/OpenAgentsLocalProvider.swift` (FM delegate tool)

### Test Files

- **AgentCoordinator Tests**: `ios/OpenAgentsCore/Tests/OpenAgentsCoreTests/Orchestration/AgentCoordinatorTests.swift`
- **Decision Engine Tests**: `ios/OpenAgentsCore/Tests/OpenAgentsCoreTests/Orchestration/DecisionEngineTests.swift`
- **Task Queue Tests**: `ios/OpenAgentsCore/Tests/OpenAgentsCoreTests/Orchestration/TaskQueueTests.swift`
- **Orchestration Config Tests**: `ios/OpenAgentsCore/Tests/OpenAgentsCoreTests/Orchestration/OrchestrationConfigTests.swift`
- **Bridge RPC Tests**: `ios/OpenAgentsCore/Tests/OpenAgentsCoreTests/DesktopBridge/OrchestrateRPCTests.swift`

### Closed Issues/PRs

- **#1465**: Wire user reply routing for conversational orchestration setup (CLOSED 2025-11-09)
- **#1464**: Conversational orchestration setup via orchestrator agent (CLOSED 2025-11-09)
- **#1462**: Orchestration aiming UI (CLOSED 2025-11-09)
- **#1452**: ACPExt capability gating for orchestrate.explore.* (CLOSED 2025-11-08)
- **#1436**: Refactor ExploreOrchestrator into state-machine reducers (CLOSED 2025-11-08)
- **#1418**: Phase 2 FM orchestrator with ACP tool streaming (CLOSED 2025-11-06)

---

## Conclusion

The OpenAgents codebase is **well-positioned for overnight orchestration**. Approximately **75% of the required infrastructure exists and is production-ready**. The delegation flow from Foundation Models to Codex is working, tested, and supports concurrent delegations.

The critical remaining work is:
1. **System constraint enforcement** (battery, WiFi) - 1-2 days
2. **PR automation service** (GitHub integration) - 2-3 days
3. **Process keep-alive** (overnight runs) - 1 day
4. **Integration testing** - 1 day

**Total remaining effort**: **5-7 days for minimal viable overnight orchestration**.

The architecture is sound, the components are well-tested, and the vision is clear. The next step is to implement the P0 gaps (constraint checking, PR automation, keep-alive) and ship Phase 1.

**Recommended Action**: Focus on Phase 1 (minimal viable overnight) before expanding to manifests or iOS monitoring. Prove the overnight value proposition with a simple, reliable implementation.

---

**Audit Date**: 2025-11-10 22:20
**Next Review**: After Phase 1 implementation (estimated 2025-11-17)
