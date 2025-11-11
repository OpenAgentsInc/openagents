# Orchestration — Architecture

**Status**: Phase 1 Implemented (2025-11-10)
**Version**: 1.0

This document describes the overnight orchestration architecture that enables autonomous coding sessions with Codex delegation.

## Overview

The overnight orchestration system allows OpenAgents to run autonomous coding sessions on macOS while you sleep. The system:

1. **Schedules** orchestration runs based on cron expressions and time windows
2. **Enforces constraints** (battery, WiFi) before running
3. **Delegates** to Codex or Claude Code via existing agent providers
4. **Prevents sleep** during active orchestration
5. **Tracks metrics** for monitoring and debugging

## Architecture Diagram

```
┌─────────────────────────────────────────────────────────────┐
│                      macOS Desktop                           │
├─────────────────────────────────────────────────────────────┤
│                                                               │
│  ┌─────────────────┐         ┌──────────────────┐           │
│  │ SchedulerService│────────▶│ AgentCoordinator │           │
│  │ • Cron timer    │         │ • runCycle()     │           │
│  │ • Constraints ✅ │         │ • DecisionEngine │           │
│  │ • Time window ✅ │         │ • TaskQueue      │           │
│  │ • Jitter ✅      │         └──────────────────┘           │
│  │ • Keep-alive ✅  │                  │                      │
│  └─────────────────┘                  │                      │
│          │                            ▼                      │
│          │                    ┌──────────────────┐           │
│          │                    │   TaskQueue      │           │
│          │                    │   (SQLite)       │           │
│          │                    └──────────────────┘           │
│          │                            │                      │
│          ▼                            ▼                      │
│  ┌─────────────────────────────────────────────┐            │
│  │      ConstraintCheckers ✅                   │            │
│  │  • PluggedInChecker (IOKit)                 │            │
│  │  • WiFiOnlyChecker (NWPathMonitor)          │            │
│  │  • CPUChecker (stub - Phase 2)              │            │
│  │  • DoNotDisturbChecker (stub - Phase 2)     │            │
│  │  • UserActivityChecker (stub - Phase 2)     │            │
│  └─────────────────────────────────────────────┘            │
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
│  │    • Broadcast to bridge clients            │            │
│  │    • Session mapping for delegation         │            │
│  └─────────────────────────────────────────────┘            │
│                                                               │
│  ✅ Codex handles PR creation autonomously                   │
│  ✅ All delegation via existing ACP protocol                 │
│                                                               │
└───────────────────────────────────────────────────────────────┘
           │
           │ (WebSocket Bridge)
           ▼
┌───────────────────────────────────────────────────────────────┐
│                        iOS Device                             │
├───────────────────────────────────────────────────────────────┤
│  ┌─────────────────────────────────────────────┐              │
│  │   Current UI: Chat + Delegation Cards       │              │
│  │   (Overnight monitoring UI - Phase 2)       │              │
│  └─────────────────────────────────────────────┘              │
└───────────────────────────────────────────────────────────────┘
```

**Legend**: ✅ = Implemented, ⏳ = Stub/Phase 2

## Components

### 1. SchedulerService (✅ Implemented)

**File**: `ios/OpenAgentsCore/Sources/OpenAgentsCore/Orchestration/SchedulerService.swift`

**Purpose**: Timer-based orchestration with constraint checking and sleep prevention.

**Key Features**:
- Cron expression support via `SchedulePreview`
- Time window enforcement (handles midnight crossing)
- Random jitter application (prevents thundering herd)
- Constraint checking before each cycle
- Process keep-alive using IOKit sleep assertions
- Metrics tracking (cycle count, last run time)

**State Machine**:
```
idle → running(nextWake) → [constraints check] → trigger
                         ↘ [failed] → paused(reason) → retry or skip
                         ↘ [cancelled] → stopped
```

**Configuration**:
```swift
let config = OrchestrationConfig(
    workspaceRoot: "/path/to/workspace",
    schedule: Schedule(
        expression: "*/30 1-5 * * *",  // Every 30 min, 1am-5am
        windowStart: "01:00",
        windowEnd: "05:00",
        jitterMs: 300000,               // 5 min jitter
        onMissed: "skip"
    ),
    constraints: Constraints(
        pluggedIn: true,
        wifiOnly: true
    ),
    timeBudgetSec: 1800,                // 30 min per cycle
    maxConcurrent: 2
)

scheduler.configure(config: config) {
    // Trigger orchestration cycle
    await coordinator.runCycle()
}

scheduler.start()
```

### 2. ConstraintCheckers (✅ Implemented)

**File**: `ios/OpenAgentsCore/Sources/OpenAgentsCore/Orchestration/ConstraintCheckers.swift`

**Purpose**: Check system constraints before orchestration runs.

**Checkers**:

1. **PluggedInChecker** (✅ Implemented)
   - Uses IOKit `IOPSCopyPowerSourcesInfo`
   - Checks if on AC power (`kIOPSACPowerValue`)
   - Returns `false` if on battery

2. **WiFiOnlyChecker** (✅ Implemented)
   - Uses `NWPathMonitor`
   - Checks for WiFi interface type
   - 2-second timeout to prevent hanging
   - Returns `false` if on cellular or not connected

3. **CPUChecker** (⏳ Stub - Phase 2)
   - Placeholder for CPU usage monitoring
   - Currently returns `true` (always pass)
   - Full implementation requires `host_processor_info` deltas

4. **DoNotDisturbChecker** (⏳ Stub - Phase 2)
   - Placeholder for DND detection
   - Currently returns `true` (assume DND off)
   - Full implementation requires `DistributedNotificationCenter`

5. **UserActivityChecker** (⏳ Stub - Phase 2)
   - Checks if user is actively using the computer
   - Uses `NSWorkspace.shared.frontmostApplication`
   - Currently allows if our app is frontmost

**Factory Pattern**:
```swift
let checkers = ConstraintCheckerFactory.createCheckers(from: config.constraints)

let allSatisfied = await ConstraintCheckerFactory.checkAll(from: config.constraints)
```

### 3. AgentCoordinator (✅ Existing)

**File**: `ios/OpenAgentsCore/Sources/OpenAgentsCore/Orchestration/AgentCoordinator.swift`

**Purpose**: Core orchestration loop that delegates to agents.

**Flow**:
1. Check for pending tasks in TaskQueue
2. If no pending tasks, use DecisionEngine to decide next task
3. Execute task using appropriate agent from AgentRegistry
4. Monitor completion and update TaskQueue status
5. Report metrics

**No changes required** - already production-ready.

### 4. DecisionEngine (✅ Existing)

**File**: `ios/OpenAgentsCore/Sources/OpenAgentsCore/Orchestration/DecisionEngine.swift`

**Purpose**: Analyze session history and decide what task to work on next.

**Decision Logic** (Heuristic-based):
- **Refactor path**: Top file touched >20x + "refactor" intent → Claude Code
- **Tests path**: No refactor signal → Codex
- **Fallback**: Exploratory analysis

**No changes required** - works with existing SessionAnalyzeTool.

### 5. TaskQueue (✅ Existing)

**File**: `ios/OpenAgentsCore/Sources/OpenAgentsCore/Orchestration/TaskQueue.swift`

**Purpose**: Persistent task storage with lifecycle management.

**Features**:
- SQLite-backed via Tinyvex
- Task statuses: pending, in_progress, completed, failed, cancelled
- Deduplication via operation hash
- FIFO with priority support

**No changes required** - production-ready.

### 6. ConfigLoader (✅ New)

**File**: `ios/OpenAgentsCore/Sources/OpenAgentsCore/Orchestration/ConfigLoader.swift`

**Purpose**: Load orchestration configs from JSON files.

**Features**:
- JSON parsing with snake_case conversion
- Variable expansion (`$WORKSPACE`, `$HOME`, `$USER`)
- Validation before returning config
- Save configs back to JSON

**Usage**:
```swift
let config = try OrchestrationConfig.load(from: "~/nightly-refactor.json")

// Or from string
let config = try OrchestrationConfig.loadFromString(jsonString)

// Save
try config.save(to: "~/my-config.json")
```

## Sequence Diagrams

### Overnight Orchestration Cycle

```
┌──────────┐         ┌─────────────┐       ┌──────────────┐       ┌────────────┐
│Scheduler │         │Constraints  │       │Coordinator   │       │Agent       │
│Service   │         │Checkers     │       │              │       │(Codex)     │
└────┬─────┘         └──────┬──────┘       └──────┬───────┘       └─────┬──────┘
     │                      │                      │                     │
     │ [1:00 AM]            │                      │                     │
     │ Wake up              │                      │                     │
     ├──────────────────────┤                      │                     │
     │ Check constraints    │                      │                     │
     │────────────────────▶ │                      │                     │
     │                      │                      │                     │
     │ ✅ All satisfied     │                      │                     │
     │◀─────────────────────┤                      │                     │
     │                      │                      │                     │
     │ Trigger cycle        │                      │                     │
     │──────────────────────────────────────────▶  │                     │
     │                      │                      │                     │
     │                      │      runCycle()      │                     │
     │                      │                      ├──────────────────┐  │
     │                      │                      │ Decision:        │  │
     │                      │                      │ "Refactor error  │  │
     │                      │                      │ handling"        │  │
     │                      │                      │◀─────────────────┘  │
     │                      │                      │                     │
     │                      │   Delegate to Codex  │                     │
     │                      │                      │────────────────────▶│
     │                      │                      │                     │
     │                      │                      │   [Codex works]     │
     │                      │                      │   • Refactors code  │
     │                      │                      │   • Runs tests      │
     │                      │                      │   • Creates PR      │
     │                      │                      │                     │
     │                      │      Task completed  │                     │
     │                      │                      │◀────────────────────│
     │                      │                      │                     │
     │ Cycle complete       │                      │                     │
     │◀─────────────────────────────────────────── │                     │
     │                      │                      │                     │
     │ [Sleep 30 min]       │                      │                     │
     │                      │                      │                     │
     │ [1:30 AM]            │                      │                     │
     │ Wake up...           │                      │                     │
     │                      │                      │                     │
```

### Constraint Check Flow

```
┌──────────┐         ┌─────────────┐       ┌──────────────┐
│Scheduler │         │PluggedIn    │       │WiFiOnly      │
│Service   │         │Checker      │       │Checker       │
└────┬─────┘         └──────┬──────┘       └──────┬───────┘
     │                      │                      │
     │ checkConstraints()   │                      │
     ├──────────────────────┤                      │
     │ check()              │                      │
     │─────────────────────▶│                      │
     │                      │                      │
     │  [IOKit API]         │                      │
     │  kIOPSACPowerValue   │                      │
     │                      │                      │
     │ true (plugged in) ✅ │                      │
     │◀─────────────────────┤                      │
     │                      │                      │
     │──────────────────────────────────────────┤  │
     │ check()              │                   │  │
     │──────────────────────────────────────────┼─▶│
     │                      │                   │  │
     │                      │ [NWPathMonitor]   │  │
     │                      │ usesInterfaceType │  │
     │                      │                   │  │
     │ true (on WiFi) ✅    │                   │  │
     │◀─────────────────────────────────────────┼──┤
     │                      │                   │  │
     │ [All constraints satisfied]              │  │
     │                      │                   │  │
```

## Configuration

### JSON Config Format

See `docs/overnight/examples/nightly-refactor.json` for a complete example.

**Required fields**:
- `id`: Unique identifier
- `workspace_root`: Path to workspace (supports `$WORKSPACE`, `$HOME`, `$USER`)
- `schedule`: Cron expression, time window, jitter, catch-up policy
- `constraints`: Battery and network requirements
- `time_budget_sec`: Max time per cycle (900-7200 seconds)
- `max_concurrent`: Max concurrent tasks (1-4)

**Optional fields**:
- `goals`: User-specified goals (biases DecisionEngine)
- `agent_preferences`: Prefer/allow lists for agents
- `focus`: Include/exclude globs for file filtering
- `pr_automation`: PR settings (enabled, branch prefix, etc.)

### Schedule Configuration

**Cron Expression** (5-field format):
```
*/30 1-5 * * *
│    │   │ │ │
│    │   │ │ └─ day of week (0-7, 0=Sunday)
│    │   │ └─── month (1-12)
│    │   └───── day of month (1-31)
│    └───────── hour (0-23)
└────────────── minute (0-59, */N for intervals)
```

**Examples**:
- `*/30 1-5 * * *` - Every 30 minutes, 1am-5am
- `0 2 * * *` - Daily at 2am
- `0 */2 * * *` - Every 2 hours

**Time Window**:
- `windowStart`: "HH:mm" (24-hour format)
- `windowEnd`: "HH:mm"
- Supports cross-midnight (e.g., "22:00" to "02:00")

**Jitter**:
- `jitterMs`: Random delay in milliseconds (0-N)
- Applied after scheduled time
- Prevents thundering herd if multiple devices use same schedule

**Catch-Up Policy**:
- `skip`: Skip missed cycles (default)
- `run_once_at_next_opportunity`: Run once when constraints satisfied

### Constraint Configuration

**pluggedIn** (`bool`):
- `true`: Only run when on AC power
- `false`: Run on battery or AC

**wifiOnly** (`bool`):
- `true`: Only run on WiFi (not cellular)
- `false`: Run on any network

**Phase 2 constraints** (not yet enforced):
- `cpuMaxPercentage`: Max CPU usage threshold
- `respectDnd`: Skip if Do Not Disturb is active
- `suspendIfActive`: Skip if user is actively using computer

## Testing

### Unit Tests

**ConstraintCheckersTests.swift**:
- Test each checker individually
- Verify protocol conformance
- Test factory pattern

**OrchestrationConfigTests.swift** (existing):
- Test config validation
- Test serialization/deserialization

### Integration Tests

**OvernightOrchestrationIntegrationTests.swift**:
- Compressed overnight run (minutes instead of hours)
- Constraint checking integration
- Time window enforcement
- Jitter application
- Sleep prevention
- Metrics tracking
- Integration with AgentCoordinator

**Test Execution**:
```bash
cd ios
xcodebuild test \
  -workspace OpenAgents.xcworkspace \
  -scheme OpenAgents \
  -destination 'platform=macOS' \
  -only-testing:OpenAgentsCoreTests/OvernightOrchestrationIntegrationTests
```

## Usage

### Starting Overnight Orchestration

```swift
// 1. Load config
let config = try OrchestrationConfig.load(from: "~/nightly-refactor.json")

// 2. Create components
let scheduler = SchedulerService()
let coordinator = AgentCoordinator(...)

// 3. Configure scheduler
await scheduler.configure(config: config) {
    await coordinator.runCycle()
}

// 4. Start scheduler
await scheduler.start()

// Scheduler now runs overnight, waking every 30 min (1am-5am)
// Checks constraints before each cycle
// Prevents system sleep
// Delegates to Codex as configured
```

### Stopping Overnight Orchestration

```swift
await scheduler.stop()
// Releases sleep assertion
// Cancels pending cycles
// Allows system sleep
```

### Monitoring

```swift
// Get status
let status = await scheduler.status()
// .idle, .running(nextWake), .paused(reason), .stopped

// Get metrics
let metrics = await scheduler.metrics()
// {
//   "cycle_count": 8,
//   "state": "running",
//   "last_run_time": "2025-11-10T03:30:00Z",
//   "seconds_since_last_run": 1800
// }
```

## Limitations (Phase 1)

1. **No PR automation** - Codex handles PRs itself
2. **No iOS monitoring UI** - macOS only for now
3. **No upgrade manifests** - Configs are JSON only, no operations registry
4. **Stub constraint checkers** - CPU, DND, UserActivity not fully implemented
5. **No multi-device coordination** - Single macOS instance only
6. **No Nostr marketplace** - No manifest publishing/discovery

## Future Work (Phase 2+)

1. **iOS Monitoring UI** - Real-time task queue, decision rationale, manual controls
2. **Advanced constraints** - CPU monitoring, DND detection, user activity
3. **Upgrade manifests** - Full declarative pipeline runtime
4. **FM-based decisions** - Replace heuristics with Foundation Models analysis
5. **Multi-device coordination** - iOS initiates, macOS executes
6. **Nostr marketplace** - Publish/discover orchestration configs

## References

### Implementation Files

- `SchedulerService.swift` - Timer and orchestration loop
- `ConstraintCheckers.swift` - System constraint checking
- `ConfigLoader.swift` - JSON config loading
- `AgentCoordinator.swift` - Core orchestration (existing)
- `DecisionEngine.swift` - Task decision logic (existing)
- `TaskQueue.swift` - Persistent task storage (existing)
- `OrchestrationConfig.swift` - Configuration model (existing)

### Test Files

- `ConstraintCheckersTests.swift` - Constraint checker tests
- `OvernightOrchestrationIntegrationTests.swift` - End-to-end tests
- `AgentCoordinatorTests.swift` - Coordinator tests (existing)
- `DecisionEngineTests.swift` - Decision logic tests (existing)
- `TaskQueueTests.swift` - Task queue tests (existing)

### Documentation

- `README.md` - Overview and demo flow
- `plan.md` - High-level implementation plan
- `audits/20251110/2220/overnight-orchestration-audit.md` - Implementation audit
- `audits/20251110/2220/next-steps.md` - Actionable next steps

---

**Last Updated**: 2025-11-10
**Status**: Phase 1 Complete
**Next Milestone**: Integration testing and validation
