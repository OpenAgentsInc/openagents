import Foundation
import LocalAuthentication
import Combine

@MainActor
final class AppLockManager: ObservableObject {
    @Published var requiresUnlock: Bool {
        didSet {
            UserDefaults.standard.set(requiresUnlock, forKey: requiresUnlockKey)
        }
    }

    @Published var isUnlocked = false

    private let requiresUnlockKey = "app.requires.unlock"

    init() {
        requiresUnlock = UserDefaults.standard.bool(forKey: requiresUnlockKey)
        if !requiresUnlock {
            isUnlocked = true
        }
    }

    func unlockIfNeeded() async {
        if !requiresUnlock {
            isUnlocked = true
            return
        }

        await authenticate()
    }

    func lock() {
        if requiresUnlock {
            isUnlocked = false
        }
    }

    func authenticate() async {
        let context = LAContext()
        var error: NSError?

        guard context.canEvaluatePolicy(.deviceOwnerAuthentication, error: &error) else {
            // Fallback to unlocked if device auth is unavailable.
            isUnlocked = true
            return
        }

        do {
            let ok = try await context.evaluatePolicy(
                .deviceOwnerAuthentication,
                localizedReason: "Unlock Inbox Autopilot"
            )
            isUnlocked = ok
        } catch {
            isUnlocked = false
        }
    }
}
