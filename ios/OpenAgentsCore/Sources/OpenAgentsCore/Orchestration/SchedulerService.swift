#if os(macOS)
import Foundation
import IOKit.pwr_mgt

/// A lightweight scheduler that triggers orchestration runs based on an OrchestrationConfig.Schedule.
/// Uses SchedulePreview to compute the next run time and sleeps until then.
/// Enforces constraints (battery, WiFi) and time windows before triggering.
/// Prevents system sleep during active orchestration runs.
public actor SchedulerService {
    // MARK: - State

    public enum State: Equatable {
        case idle
        case running(nextWake: Date?)
        case paused(reason: String)
        case stopped
    }

    // MARK: - Properties

    private var state: State = .idle
    private var task: Task<Void, Never>? = nil
    private var config: OrchestrationConfig? = nil
    private var trigger: (() async -> Void)? = nil
    private var sleepAssertionID: IOPMAssertionID = 0
    private var lastRunTime: Date?
    private var cycleCount: Int = 0

    // MARK: - Initialization

    public init() {}

    // MARK: - Configuration

    /// Configure the scheduler with an orchestration config and trigger callback
    /// - Parameters:
    ///   - config: Orchestration configuration with schedule and constraints
    ///   - trigger: Async callback to invoke when orchestration should run
    public func configure(config: OrchestrationConfig, trigger: @escaping () async -> Void) {
        self.config = config
        self.trigger = trigger
    }

    // MARK: - Control

    /// Start the scheduler
    /// Prevents system sleep and begins the orchestration loop
    public func start() {
        guard task == nil, let cfg = config, let trigger = trigger else {
            OpenAgentsLog.orchestration.warning("Cannot start scheduler: missing config or trigger")
            return
        }

        OpenAgentsLog.orchestration.info("Starting scheduler with schedule: \(cfg.schedule.expression)")

        // Prevent system sleep during orchestration
        preventSleep()

        task = Task { [weak self] in
            guard let self else { return }

            while !Task.isCancelled {
                // Compute next wake time
                let next = SchedulerService.nextWake(for: cfg.schedule, from: Date())
                await self.setState(.running(nextWake: next))

                guard let next else {
                    OpenAgentsLog.orchestration.warning("No next wake time computed, stopping")
                    break
                }

                // Apply jitter if configured
                let jitteredNext = await self.applyJitter(to: next, jitterMs: cfg.schedule.jitterMs)

                // Sleep until next wake time
                let delay = max(0, jitteredNext.timeIntervalSinceNow)
                OpenAgentsLog.orchestration.debug("Sleeping for \(delay)s until \(jitteredNext)")

                do {
                    try await Task.sleep(nanoseconds: UInt64(delay * 1_000_000_000))
                } catch {
                    OpenAgentsLog.orchestration.info("Sleep interrupted, stopping")
                    break
                }

                if Task.isCancelled { break }

                // Check time window
                let withinWindow = await self.isWithinTimeWindow(cfg.schedule)
                if !withinWindow {
                    OpenAgentsLog.orchestration.info("Outside time window, skipping cycle")
                    continue
                }

                // Check constraints
                let constraintsSatisfied = await self.checkConstraints(cfg.constraints)
                if !constraintsSatisfied {
                    let catchUpPolicy = cfg.schedule.onMissed ?? "skip"
                    if catchUpPolicy == "skip" {
                        OpenAgentsLog.orchestration.info("Constraints not satisfied, skipping cycle (policy: skip)")
                        continue
                    } else {
                        // run_once_at_next_opportunity: pause and retry
                        OpenAgentsLog.orchestration.info("Constraints not satisfied, pausing (policy: run_once)")
                        await self.setState(.paused(reason: "Waiting for constraints"))

                        // Wait 5 minutes and retry
                        do {
                            try await Task.sleep(nanoseconds: 5 * 60 * 1_000_000_000)
                        } catch {
                            break
                        }

                        // Recheck constraints
                        let retriedCheck = await self.checkConstraints(cfg.constraints)
                        if !retriedCheck {
                            OpenAgentsLog.orchestration.info("Constraints still not satisfied after retry, skipping")
                            continue
                        }

                        OpenAgentsLog.orchestration.info("Constraints satisfied after retry, continuing")
                    }
                }

                // All checks passed, trigger orchestration
                await self.recordCycleStart()
                let currentCycle = await self.cycleCount
                OpenAgentsLog.orchestration.info("Triggering orchestration cycle #\(currentCycle)")

                await trigger()

                OpenAgentsLog.orchestration.info("Orchestration cycle #\(currentCycle) completed")
            }

            await self.setState(.stopped)
        }
    }

    /// Stop the scheduler
    /// Cancels the current task and allows system sleep
    public func stop() {
        OpenAgentsLog.orchestration.info("Stopping scheduler")

        task?.cancel()
        task = nil
        state = .stopped

        // Allow system sleep
        allowSleep()
    }

    /// Get current scheduler status
    /// - Returns: Current state
    public func status() -> State {
        return state
    }

    /// Get scheduler metrics
    /// - Returns: Dictionary of metrics (cycle_count, last_run_time, uptime)
    public func metrics() -> [String: Any] {
        var result: [String: Any] = [
            "cycle_count": cycleCount,
            "state": stateString(state)
        ]

        if let lastRun = lastRunTime {
            result["last_run_time"] = ISO8601DateFormatter().string(from: lastRun)
            result["seconds_since_last_run"] = Date().timeIntervalSince(lastRun)
        }

        return result
    }

    // MARK: - Private Helpers

    private func setState(_ s: State) {
        self.state = s
    }

    private func recordCycleStart() {
        self.cycleCount += 1
        self.lastRunTime = Date()
    }

    private func stateString(_ state: State) -> String {
        switch state {
        case .idle: return "idle"
        case .running: return "running"
        case .paused: return "paused"
        case .stopped: return "stopped"
        }
    }

    /// Check if current time is within configured time window
    private func isWithinTimeWindow(_ schedule: OrchestrationConfig.Schedule) -> Bool {
        guard let windowStart = schedule.windowStart,
              let windowEnd = schedule.windowEnd else {
            // No window configured, always allow
            return true
        }

        let now = Date()
        let calendar = Calendar.current
        let components = calendar.dateComponents([.hour, .minute], from: now)

        guard let hour = components.hour, let minute = components.minute else {
            return false
        }

        // Parse window times
        guard let start = parseTime(windowStart),
              let end = parseTime(windowEnd) else {
            OpenAgentsLog.orchestration.warning("Failed to parse time window, allowing")
            return true
        }

        let currentMinutes = hour * 60 + minute
        let startMinutes = start.hour * 60 + start.minute
        let endMinutes = end.hour * 60 + end.minute

        // Handle cross-midnight window (e.g., 22:00 - 02:00)
        if startMinutes >= endMinutes {
            return currentMinutes >= startMinutes || currentMinutes <= endMinutes
        } else {
            return currentMinutes >= startMinutes && currentMinutes <= endMinutes
        }
    }

    private func parseTime(_ time: String) -> (hour: Int, minute: Int)? {
        let parts = time.split(separator: ":")
        guard parts.count == 2,
              let hour = Int(parts[0]),
              let minute = Int(parts[1]),
              hour >= 0, hour < 24,
              minute >= 0, minute < 60 else {
            return nil
        }
        return (hour, minute)
    }

    /// Apply random jitter to wake time
    private func applyJitter(to date: Date, jitterMs: Int?) -> Date {
        guard let jitterMs = jitterMs, jitterMs > 0 else {
            return date
        }

        // Random jitter between 0 and jitterMs
        let jitterSeconds = Double.random(in: 0...Double(jitterMs) / 1000.0)
        return date.addingTimeInterval(jitterSeconds)
    }

    /// Check if all constraints are satisfied
    private func checkConstraints(_ constraints: OrchestrationConfig.Constraints) async -> Bool {
        #if os(macOS)
        return await ConstraintCheckerFactory.checkAll(from: constraints)
        #else
        return true
        #endif
    }

    /// Prevent system sleep during orchestration
    private func preventSleep() {
        guard sleepAssertionID == 0 else {
            return
        }

        var assertionID: IOPMAssertionID = 0
        let result = IOPMAssertionCreateWithName(
            kIOPMAssertionTypePreventUserIdleSystemSleep as CFString,
            IOPMAssertionLevel(kIOPMAssertionLevelOn),
            "OpenAgents Overnight Orchestration" as CFString,
            &assertionID
        )

        if result == kIOReturnSuccess {
            sleepAssertionID = assertionID
            OpenAgentsLog.orchestration.info("Sleep prevention enabled (assertion ID: \(assertionID))")
        } else {
            OpenAgentsLog.orchestration.error("Failed to prevent sleep: \(result)")
        }
    }

    /// Allow system sleep
    private func allowSleep() {
        guard sleepAssertionID != 0 else {
            return
        }

        let assertionID = self.sleepAssertionID
        let result = IOPMAssertionRelease(assertionID)
        if result == kIOReturnSuccess {
            OpenAgentsLog.orchestration.info("Sleep prevention disabled (assertion ID: \(assertionID))")
            sleepAssertionID = 0
        } else {
            OpenAgentsLog.orchestration.error("Failed to release sleep assertion: \(result)")
        }
    }

    /// Compute the next wake based on schedule. Returns nil if cannot determine.
    public static func nextWake(for schedule: OrchestrationConfig.Schedule, from: Date) -> Date? {
        SchedulePreview.nextRuns(schedule: schedule, count: 1, from: from).first
    }
}

#endif

