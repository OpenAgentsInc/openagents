// Firm reason: 6 (macOS IOKit IOPMAssertion keep-awake during active agent turns; GitHub #14025).
import Foundation
import IOKit.pwr_mgt

/// macOS OS Keep-Awake manager backed by IOKit `IOPMAssertionCreateWithName` / `IOPMAssertionRelease`.
/// Asserted automatically while any agent turn or `/goal` is running (when enabled) and released immediately when all runs settle.
final class KeepAwakeManager {
    static let shared = KeepAwakeManager()
    static let enabledKey = "keepAwakeDuringRuns"

    static var enabled: Bool {
        get { UserDefaults.standard.object(forKey: enabledKey) as? Bool ?? true }
        set { UserDefaults.standard.set(newValue, forKey: enabledKey) }
    }

    private var assertionID: IOPMAssertionID = 0
    private(set) var isActive = false

    func update(active: Bool) {
        if active && !isActive {
            var id: IOPMAssertionID = 0
            let result = IOPMAssertionCreateWithName(
                kIOPMAssertionTypePreventUserIdleSystemSleep as CFString,
                IOPMAssertionLevel(kIOPMAssertionLevelOn),
                "OpenAgents Desktop active agent run" as CFString,
                &id
            )
            if result == kIOReturnSuccess {
                assertionID = id
                isActive = true
            }
        } else if !active && isActive {
            release()
        }
    }

    func release() {
        guard isActive else { return }
        IOPMAssertionRelease(assertionID)
        assertionID = 0
        isActive = false
    }
}
