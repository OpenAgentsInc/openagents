# Issue #001: Implement SchedulerService

**Component**: Orchestration Layer - Scheduler
**Priority**: P0 (Critical Path)
**Estimated Effort**: 2-3 days (scoped for demo)
**Dependencies**: None
**Assignee**: TBD

---

## Overview

Implement `SchedulerService`, a macOS-only background service that provides time-based orchestration wake-up with minimal constraint checking. This is the entry point for the overnight agents system.

**Scope for Demo**: Implement cron + window + jitter + plugged_in + wifi_only only. Defer DND/CPU/user-activity to post-demo.

**Location**: `ios/OpenAgentsCore/Sources/OpenAgentsCore/Orchestration/SchedulerService.swift`

---

## Requirements

### Functional Requirements

1. **Cron Expression Parsing**
   - Support standard 5-field cron syntax: `minute hour day month weekday`
   - Examples:
     - `*/30 1-5 * * *` - Every 30 minutes, 1am-5am
     - `0 2 * * *` - Daily at 2am
     - `0 */2 * * *` - Every 2 hours
   - Use existing Swift cron parsing library or implement parser

2. **Time Window Enforcement**
   - Optional `window` field: `{start: "01:00", end: "05:00"}`
   - Only execute within window, even if cron expression matches outside
   - Handle edge cases (window crossing midnight)

3. **Jitter Support**
   - Random delay 0-N seconds after scheduled time
   - Prevents thundering herd if multiple devices use same schedule
   - Configurable per manifest (default: 0)

4. **Constraint Checking (Demo Scope)**
   - `plugged_in: bool` - Only run when on AC power (IOKit battery API)
   - `wifi_only: bool` - Only run on WiFi, not cellular (NWPathMonitor)
   - **Post-Demo**: `cpu_max_percentage`, `respect_dnd`, `suspend_if_active` (defer these)

5. **Catch-Up Policy**
   - `run_once_at_next_opportunity`: If missed due to constraints, run once when available
   - `skip`: If missed, skip and wait for next scheduled time

6. **State Management**
   - Track current state: idle, running, paused, stopped
   - Persist last run time and next scheduled time in Tinyvex DB (new table: `overnight_scheduler_state`)
   - Observable state for UI updates via AsyncStream

7. **Graceful Shutdown**
   - `stop()` method: finish current task, then stop
   - Don't interrupt mid-execution
   - Clean up timers and resources

---

### Non-Functional Requirements

1. **Performance**
   - Constraint checks must complete within 500ms
   - Timer loop should use minimal CPU when idle
   - No battery drain (design for overnight runs)

2. **Reliability**
   - Must survive macOS sleep/wake cycles
   - Must resume after app restart (persist state)
   - Must handle time zone changes

3. **Testability**
   - All methods async/await for easy testing
   - Dependency injection for constraint checkers
   - Mock-friendly interfaces

---

## Implementation Spec

### Interface

```swift
/// macOS-only background scheduler service
@available(macOS 13.0, *)
actor SchedulerService {
    /// Start scheduler with upgrade manifest
    /// - Parameter upgrade: Upgrade manifest containing schedule config
    /// - Throws: SchedulerError if configuration invalid or constraints not met
    func start(upgrade: UpgradeManifest) async throws

    /// Stop scheduler gracefully (finish current task)
    func stop() async

    /// Check if all constraints are satisfied
    /// - Parameter constraints: Schedule constraints to check
    /// - Returns: True if all constraints satisfied
    func checkConstraints(_ constraints: ScheduleConstraints) async -> Bool

    /// Calculate next wake time with jitter
    /// - Parameters:
    ///   - from: Current time
    ///   - cron: Cron expression
    ///   - jitter: Max random delay in seconds
    /// - Returns: Next wake time, or nil if no match
    func nextWakeTime(from: Date, cron: String, jitter: Int) -> Date?

    /// Check if current time is within time window
    /// - Parameters:
    ///   - time: Time to check
    ///   - window: Time window (start/end as "HH:mm")
    /// - Returns: True if within window
    func isWithinWindow(_ time: Date, window: TimeWindow?) -> Bool

    /// Current scheduler state
    var state: SchedulerState { get async }

    /// Observable stream of state changes
    var stateUpdates: AsyncStream<SchedulerState> { get }
}

enum SchedulerState: Equatable {
    case idle
    case running(nextWake: Date)
    case paused(reason: String)
    case stopped
}

enum SchedulerError: Error {
    case invalidCronExpression(String)
    case invalidTimeWindow(String)
    case upgradeExecutorFailed(Error)
    case constraintsNotMet([String])
}
```

### Constraint Checkers

```swift
protocol ConstraintChecker {
    func check() async -> Bool
}

actor PluggedInChecker: ConstraintChecker {
    func check() async -> Bool {
        // Use IOKit: IOPMCopyBatteryInfo
        // Return true if on AC power
        let snapshot = IOPSCopyPowerSourcesInfo().takeRetainedValue()
        let sources = IOPSCopyPowerSourcesList(snapshot).takeRetainedValue() as Array

        for source in sources {
            if let description = IOPSGetPowerSourceDescription(snapshot, source).takeUnretainedValue() as? [String: Any],
               let powerSourceState = description[kIOPSPowerSourceStateKey] as? String {
                return powerSourceState == kIOPSACPowerValue
            }
        }

        return false
    }
}

actor WiFiOnlyChecker: ConstraintChecker {
    func check() async -> Bool {
        // Use NWPathMonitor
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

actor CPUChecker: ConstraintChecker {
    let maxPercentage: Double

    init(maxPercentage: Double) {
        self.maxPercentage = maxPercentage
    }

    func check() async -> Bool {
        // Use host_processor_info to get CPU usage
        var processorInfoCount: mach_msg_type_number_t = 0
        var processorInfo: processor_info_array_t?
        var numProcessors: natural_t = 0

        let result = host_processor_info(
            mach_host_self(),
            PROCESSOR_CPU_LOAD_INFO,
            &numProcessors,
            &processorInfo,
            &processorInfoCount
        )

        guard result == KERN_SUCCESS else { return true }

        defer {
            vm_deallocate(mach_task_self_, vm_address_t(bitPattern: processorInfo), vm_size_t(processorInfoCount))
        }

        // Calculate CPU usage
        // (simplified - actual implementation needs to track delta over time)
        return true  // TODO: Implement actual CPU usage calculation
    }
}

actor DoNotDisturbChecker: ConstraintChecker {
    func check() async -> Bool {
        // Use DistributedNotificationCenter
        // Listen for "com.apple.donotdisturb" notification
        // For now, assume DND is off
        return true  // TODO: Implement actual DND check
    }
}

actor UserActivityChecker: ConstraintChecker {
    func check() async -> Bool {
        // Use NSWorkspace to check if user is active
        guard let frontmost = NSWorkspace.shared.frontmostApplication else {
            return true  // No app is frontmost, user not active
        }

        // If frontmost app is our app, user is not "actively using" another app
        let ourBundleId = Bundle.main.bundleIdentifier ?? ""
        return frontmost.bundleIdentifier == ourBundleId
    }
}
```

### Timer Loop

```swift
func start(upgrade: UpgradeManifest) async throws {
    let schedule = upgrade.schedule

    // Validate cron expression
    guard isValidCronExpression(schedule.expression) else {
        throw SchedulerError.invalidCronExpression(schedule.expression)
    }

    // Set state to running
    await setState(.running(nextWake: Date()))

    // Main timer loop
    while await state != .stopped {
        // Calculate next wake time
        guard let nextWake = nextWakeTime(
            from: Date(),
            cron: schedule.expression,
            jitter: schedule.jitter ?? 0
        ) else {
            // No more scheduled times (shouldn't happen with cron)
            await setState(.stopped)
            break
        }

        // Update state with next wake time
        await setState(.running(nextWake: nextWake))

        // Sleep until next wake time
        let sleepDuration = nextWake.timeIntervalSinceNow
        if sleepDuration > 0 {
            try await Task.sleep(nanoseconds: UInt64(sleepDuration * 1_000_000_000))
        }

        // Check if within time window
        if let window = schedule.window {
            guard isWithinWindow(Date(), window: window) else {
                continue  // Skip this cycle, wait for next
            }
        }

        // Check constraints
        let constraintsSatisfied = await checkConstraints(schedule.constraints)
        if !constraintsSatisfied {
            if schedule.onMissed == .skip {
                continue  // Skip and wait for next cycle
            } else {
                // run_once_at_next_opportunity: wait and retry
                await setState(.paused(reason: "Waiting for constraints"))
                // TODO: Implement retry logic with backoff
                continue
            }
        }

        // Execute upgrade pipeline
        do {
            let executor = UpgradeExecutor()
            _ = try await executor.execute(
                upgrade.pipeline,
                context: ExecutionContext(
                    variables: [:],
                    workingDir: FileManager.default.currentDirectoryPath,
                    timeBudget: 3600  // 1 hour default
                )
            )
        } catch {
            // Log error but continue scheduling
            print("Upgrade execution failed: \(error)")
        }
    }
}
```

---

## Testing

### Unit Tests

**File**: `ios/OpenAgentsCore/Tests/OpenAgentsCoreTests/Orchestration/SchedulerServiceTests.swift`

1. `testCronExpressionParsing()` - Valid and invalid cron expressions
2. `testNextWakeTimeCalculation()` - Various cron patterns
3. `testJitterApplication()` - Random delay within bounds
4. `testTimeWindowEnforcement()` - Inside/outside window
5. `testConstraintChecking_PluggedIn()` - Mock IOKit
6. `testConstraintChecking_WiFi()` - Mock NWPathMonitor
7. `testConstraintChecking_CPU()` - Mock CPU usage
8. `testGracefulShutdown()` - Stop mid-execution

### Integration Tests

1. `testCompressedSchedule()` - Run with 1-min intervals
2. `testStateObservability()` - Stream state updates
3. `testResumeAfterAppRestart()` - Persist/restore state

---

## Acceptance Criteria

- [ ] Cron expression parser handles all standard 5-field patterns
- [ ] Time window enforcement works, including midnight crossing
- [ ] Jitter is applied correctly (random within 0-N seconds)
- [ ] All constraint checkers implemented and tested
- [ ] Catch-up policy (skip vs retry) works as expected
- [ ] State is observable via AsyncStream
- [ ] Graceful shutdown finishes current task before stopping
- [ ] All unit tests pass with ≥90% coverage
- [ ] Integration test with compressed schedule (1-min intervals) succeeds
- [ ] No memory leaks (Instruments check)
- [ ] Works across macOS sleep/wake cycles

---

## Notes

- Use existing Swift cron library (e.g., `SwiftCron`) or implement parser
- Constraint checkers should be dependency-injected for testing
- Consider using `AsyncTimerSequence` (Swift 5.9+) for cleaner timer loop
- State persistence: use UserDefaults or SQLite (Tinyvex)
- iOS compatibility: All constraint checks should no-op or return `true` on iOS

---

## References

- Architecture doc: `architecture.md` - SchedulerService section
- Testing plan: `testing-plan.md` - SchedulerService tests
- Apple docs:
  - IOKit power APIs: https://developer.apple.com/documentation/iokit
  - NWPathMonitor: https://developer.apple.com/documentation/network/nwpathmonitor
  - NSWorkspace: https://developer.apple.com/documentation/appkit/nsworkspace
