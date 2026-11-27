// Firm reason: 6 (iOS zero-secret local turn-completion and approval notifications + background grace window via UserNotifications and UIApplication).
import Foundation
import UIKit
import UserNotifications

@MainActor
final class TurnNotifications {
    static let shared = TurnNotifications()

    private var backgroundTaskID: UIBackgroundTaskIdentifier = .invalid
    private var requestedAuthorization = false

    private var isUITestEnvironment: Bool {
        let args = ProcessInfo.processInfo.arguments
        return args.contains("-OAResetStateForTests")
            || args.contains("-OAE2ELoopbackAllowed")
            || args.contains("-OANoLocalNetworkBrowse")
    }

    func requestAuthorizationIfNeeded() {
        guard !requestedAuthorization, !isUITestEnvironment else { return }
        requestedAuthorization = true
        UNUserNotificationCenter.current().requestAuthorization(options: [.alert, .sound]) { _, _ in }
    }

    /// Posts strictly zero-secret local notifications when the app is backgrounded.
    func notifyBackgroundEvent(kind: String, title: String) {
        guard !isUITestEnvironment else { return }
        guard UIApplication.shared.applicationState != .active else { return }

        let content = UNMutableNotificationContent()
        content.title = "OpenAgents"
        if kind == "approval" {
            content.body = "OpenAgents is waiting for your approval."
        } else {
            content.body = title.isEmpty ? "OpenAgents finished your turn." : title
        }
        content.sound = .default

        let request = UNNotificationRequest(
            identifier: "oa-turn-\(kind)-\(UUID().uuidString)",
            content: content,
            trigger: nil
        )
        UNUserNotificationCenter.current().add(request)
    }

    func beginBackgroundGracePeriod(activeTurnRunning: Bool) {
        guard activeTurnRunning, !isUITestEnvironment else { return }
        requestAuthorizationIfNeeded()
        endBackgroundGracePeriod()
        backgroundTaskID = UIApplication.shared.beginBackgroundTask(withName: "OpenAgentsActiveTurn") { [weak self] in
            Task { @MainActor in
                self?.endBackgroundGracePeriod()
            }
        }
    }

    func endBackgroundGracePeriod() {
        if backgroundTaskID != .invalid {
            UIApplication.shared.endBackgroundTask(backgroundTaskID)
            backgroundTaskID = .invalid
        }
    }
}
