#if os(macOS)
import Foundation
import IOKit.ps
import Network
import AppKit

// MARK: - Constraint Checker Protocol

/// Protocol for checking system constraints before orchestration runs
public protocol ConstraintChecker: Sendable {
    /// Check if the constraint is satisfied
    /// - Returns: True if constraint is satisfied, false otherwise
    func check() async -> Bool

    /// Human-readable name for this constraint
    var name: String { get }
}

// MARK: - Plugged In Checker

/// Checks if the device is plugged into AC power
public actor PluggedInChecker: ConstraintChecker {
    public let name: String = "plugged_in"

    public init() {}

    public func check() async -> Bool {
        // Use IOKit to check power source
        guard let snapshot = IOPSCopyPowerSourcesInfo()?.takeRetainedValue() else {
            OpenAgentsLog.orchestration.warning("Failed to get power source info, assuming unplugged")
            return false
        }

        guard let sources = IOPSCopyPowerSourcesList(snapshot)?.takeRetainedValue() as? [CFTypeRef] else {
            OpenAgentsLog.orchestration.warning("Failed to get power source list, assuming unplugged")
            return false
        }

        for source in sources {
            guard let description = IOPSGetPowerSourceDescription(snapshot, source)?.takeUnretainedValue() as? [String: Any] else {
                continue
            }

            // Check if on AC power
            if let powerSourceState = description[kIOPSPowerSourceStateKey] as? String {
                let isPluggedIn = powerSourceState == kIOPSACPowerValue
                OpenAgentsLog.orchestration.debug("Power source state: \(powerSourceState), plugged in: \(isPluggedIn)")
                return isPluggedIn
            }
        }

        // If we couldn't determine, assume unplugged for safety
        OpenAgentsLog.orchestration.warning("Could not determine power state, assuming unplugged")
        return false
    }
}

// MARK: - WiFi Only Checker

/// Checks if the device is connected to WiFi (not cellular)
public actor WiFiOnlyChecker: ConstraintChecker {
    public let name: String = "wifi_only"

    public init() {}

    public func check() async -> Bool {
        return await withCheckedContinuation { continuation in
            let monitor = NWPathMonitor()
            let queue = DispatchQueue(label: "com.openagents.wifichecker")

            var hasResumed = false

            // Set timeout to prevent hanging
            DispatchQueue.global().asyncAfter(deadline: .now() + 2.0) {
                if !hasResumed {
                    hasResumed = true
                    monitor.cancel()
                    OpenAgentsLog.orchestration.warning("WiFi check timed out, assuming not on WiFi")
                    continuation.resume(returning: false)
                }
            }

            monitor.pathUpdateHandler = { path in
                guard !hasResumed else { return }
                hasResumed = true

                let isWiFi = path.usesInterfaceType(.wifi)
                let status = path.status

                OpenAgentsLog.orchestration.debug("Network status: \(String(describing: status)), WiFi: \(isWiFi)")

                monitor.cancel()
                continuation.resume(returning: isWiFi && status == .satisfied)
            }

            monitor.start(queue: queue)
        }
    }
}

// MARK: - CPU Usage Checker

/// Checks if CPU usage is below a threshold
/// Note: This is a simplified implementation. Full implementation requires tracking deltas over time.
public actor CPUChecker: ConstraintChecker {
    public let name: String = "cpu_usage"
    private let maxPercentage: Double

    public init(maxPercentage: Double = 80.0) {
        self.maxPercentage = maxPercentage
    }

    public func check() async -> Bool {
        // TODO: Implement actual CPU usage tracking
        // This requires:
        // 1. Get host_processor_info snapshots
        // 2. Calculate deltas between snapshots
        // 3. Compute CPU usage percentage
        //
        // For Phase 1 (demo), we return true (always pass)
        // This will be implemented in Phase 2 (post-demo)

        OpenAgentsLog.orchestration.debug("CPU check not implemented, returning true (Phase 2)")
        return true
    }
}

// MARK: - Do Not Disturb Checker

/// Checks if Do Not Disturb mode is active
/// Note: This is a stub implementation. Full implementation requires DistributedNotificationCenter.
public actor DoNotDisturbChecker: ConstraintChecker {
    public let name: String = "respect_dnd"

    public init() {}

    public func check() async -> Bool {
        // TODO: Implement DND detection
        // This requires:
        // 1. Listen to com.apple.donotdisturb notifications via DistributedNotificationCenter
        // 2. Track DND state
        //
        // For Phase 1 (demo), we return true (assume DND is off)
        // This will be implemented in Phase 2 (post-demo)

        OpenAgentsLog.orchestration.debug("DND check not implemented, returning true (Phase 2)")
        return true
    }
}

// MARK: - User Activity Checker

/// Checks if user is actively using the computer
public actor UserActivityChecker: ConstraintChecker {
    public let name: String = "suspend_if_active"

    public init() {}

    public func check() async -> Bool {
        // Check if user is actively using the computer
        // by looking at the frontmost application

        guard let frontmost = NSWorkspace.shared.frontmostApplication else {
            OpenAgentsLog.orchestration.debug("No frontmost app, user not active")
            return true
        }

        // If our app is frontmost, user might be monitoring, so allow
        let ourBundleId = Bundle.main.bundleIdentifier ?? ""
        if frontmost.bundleIdentifier == ourBundleId {
            OpenAgentsLog.orchestration.debug("Our app is frontmost, allowing orchestration")
            return true
        }

        // If another app is frontmost, user is active, so suspend orchestration
        OpenAgentsLog.orchestration.debug("User is active (frontmost: \(frontmost.localizedName ?? "unknown")), suspending")
        return false
    }
}

// MARK: - Constraint Checker Factory

/// Factory for creating constraint checkers based on configuration
public enum ConstraintCheckerFactory {
    /// Create constraint checkers from orchestration constraints
    /// - Parameter constraints: Orchestration constraints from config
    /// - Returns: Array of constraint checkers to evaluate
    public static func createCheckers(from constraints: OrchestrationConfig.Constraints) -> [ConstraintChecker] {
        var checkers: [ConstraintChecker] = []

        if constraints.pluggedIn {
            checkers.append(PluggedInChecker())
        }

        if constraints.wifiOnly {
            checkers.append(WiFiOnlyChecker())
        }

        // Phase 2 constraints (not enforced in Phase 1)
        // Uncomment when implementing:
        // if let cpuMax = constraints.cpuMaxPercentage {
        //     checkers.append(CPUChecker(maxPercentage: cpuMax))
        // }
        // if constraints.respectDnd {
        //     checkers.append(DoNotDisturbChecker())
        // }
        // if constraints.suspendIfActive {
        //     checkers.append(UserActivityChecker())
        // }

        return checkers
    }

    /// Check if all constraints are satisfied
    /// - Parameter constraints: Orchestration constraints from config
    /// - Returns: True if all constraints are satisfied, false otherwise
    public static func checkAll(from constraints: OrchestrationConfig.Constraints) async -> Bool {
        let checkers = createCheckers(from: constraints)

        guard !checkers.isEmpty else {
            OpenAgentsLog.orchestration.debug("No constraints configured, allowing orchestration")
            return true
        }

        for checker in checkers {
            let satisfied = await checker.check()
            if !satisfied {
                OpenAgentsLog.orchestration.info("Constraint '\(checker.name)' not satisfied, skipping cycle")
                return false
            }
        }

        OpenAgentsLog.orchestration.debug("All \(checkers.count) constraint(s) satisfied")
        return true
    }
}

#endif
