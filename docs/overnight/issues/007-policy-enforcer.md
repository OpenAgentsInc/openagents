# Issue #007: Implement PolicyEnforcer

**Component**: Safety & Compliance
**Priority**: P1 (High)
**Estimated Effort**: 2-3 days
**Dependencies**: None
**Assignee**: TBD

---

## Overview

Enforce Foundation Models AUP compliance (DPLA §3.3.8), resource limits, filesystem permissions, time budgets.

**Location**: `ios/OpenAgentsCore/Sources/OpenAgentsCore/Orchestration/PolicyEnforcer.swift`

---

## Requirements

1. **AUP Compliance**: Check prompts for prohibited content (violence, pornography, fraud, etc.)
2. **Resource Limits**: CPU, memory, disk usage checks
3. **Filesystem Permissions**: Validate paths against whitelist/blacklist
4. **Time Budget**: Enforce per-task time limits
5. **User Overrides**: Allow manual approval for flagged content

---

## Implementation

```swift
actor PolicyEnforcer {
    private let prohibitedPatterns = [
        "violence", "pornography", "self-harm", "fraud",
        "healthcare advice", "legal advice", "financial advice",
        "identify training data", "academic textbooks", "circumvent safety"
    ]

    func checkAUP(_ prompt: String) async throws -> PolicyResult {
        let lower = prompt.lowercased()

        for pattern in prohibitedPatterns {
            if lower.contains(pattern) {
                return .denied(reason: "Potential AUP violation: \(pattern)")
            }
        }

        // Future: Use on-device classifier model
        return .allowed
    }

    func checkFileAccess(_ path: String, mode: FileAccessMode, permissions: UpgradePermissions) -> Bool {
        // Check if path is in whitelist
        if let allowed = permissions.filesystem.read, mode == .read {
            return allowed.contains { pathMatches($0, path) }
        }

        // Check if path is in blacklist
        if let excluded = permissions.filesystem.exclude {
            if excluded.contains(where: { pathMatches($0, path) }) {
                return false
            }
        }

        return true
    }

    func checkResourceLimits() async throws -> ResourceStatus {
        // Get CPU usage
        let cpuUsage = try await getCPUUsage()

        // Get memory usage
        var info = mach_task_basic_info()
        var count = mach_msg_type_number_t(MemoryLayout<mach_task_basic_info>.size) / 4
        let kerr = withUnsafeMutablePointer(to: &info) {
            $0.withMemoryRebound(to: integer_t.self, capacity: 1) {
                task_info(mach_task_self_, task_flavor_t(MACH_TASK_BASIC_INFO), $0, &count)
            }
        }

        let memoryUsage = kerr == KERN_SUCCESS ? info.resident_size : 0

        // Get disk space
        let attrs = try FileManager.default.attributesOfFileSystem(forPath: "/")
        let diskAvailable = attrs[.systemFreeSize] as? UInt64 ?? 0

        let withinLimits = cpuUsage < 0.8 && memoryUsage < 4_000_000_000 && diskAvailable > 1_000_000_000

        return ResourceStatus(
            cpuUsage: cpuUsage,
            memoryUsage: memoryUsage,
            diskAvailable: diskAvailable,
            withinLimits: withinLimits
        )
    }

    func enforceTimeBudget(_ task: OvernightTask, elapsed: TimeInterval) -> TimeBudgetResult {
        let budget = task.decision.estimatedDuration * 1.5  // 50% grace period
        let remaining = budget - elapsed

        if remaining <= 0 {
            return .exceeded
        } else if remaining < 300 {  // < 5 minutes
            return .approaching(remaining: remaining)
        } else {
            return .withinBudget
        }
    }

    private func pathMatches(_ pattern: String, _ path: String) -> Bool {
        // Simple glob matching (or use NSPredicate)
        let regexPattern = pattern
            .replacingOccurrences(of: "**", with: "DOUBLESTAR")
            .replacingOccurrences(of: "*", with: "[^/]*")
            .replacingOccurrences(of: "DOUBLESTAR", with: ".*")

        return path.range(of: "^\(regexPattern)$", options: .regularExpression) != nil
    }

    private func getCPUUsage() async throws -> Double {
        // Simplified - actual impl needs to track delta
        return 0.3  // Mock 30% usage
    }
}

enum PolicyResult {
    case allowed
    case denied(reason: String)
    case warning(message: String)
}

struct ResourceStatus {
    let cpuUsage: Double
    let memoryUsage: UInt64
    let diskAvailable: UInt64
    let withinLimits: Bool
}

enum TimeBudgetResult {
    case withinBudget
    case approaching(remaining: TimeInterval)
    case exceeded
}
```

---

## Testing

1. `testAUPChecks()` - Prohibited patterns detected
2. `testFilePermissions()` - Whitelist/blacklist logic
3. `testResourceLimits()` - CPU/memory/disk checks
4. `testTimeBudget()` - Approaching/exceeded states

---

## Acceptance Criteria

- [ ] AUP checks block prohibited content
- [ ] File permission validation works
- [ ] Resource limits enforced
- [ ] Time budget enforcement correct
- [ ] Tests pass (≥95% coverage)

---

## References

- docs/compute/apple-terms-research.md (DPLA §3.3.8)
- docs/compute/issues/phase-1-mvp/009-policy-safety-module.md
