// Push wakes are off unless this build names a relay, a push gateway, and an
// app profile (see bins/coder-ios/README.md). Rust owns registration, the
// delivery grant, and the lease. This file only asks for notification
// permission, obtains the APNs token, and passes it to Rust as lowercase hex.
import SwiftUI
import UIKit
import UserNotifications

/// The push settings this build carries in its Info.plist. Empty values,
/// the default, leave push off.
struct PushSettings {
    let relay: String
    let gateway: String
    let profile: String

    var rust: [String: String] { ["relay_url": relay, "gateway_url": gateway, "app_profile": profile] }

    static var configured: PushSettings? {
        let info = Bundle.main.infoDictionary ?? [:]
        func value(_ key: String) -> String? {
            guard let text = (info[key] as? String)?.trimmingCharacters(in: .whitespacesAndNewlines),
                  !text.isEmpty, !text.hasPrefix("$(") else { return nil }
            return text
        }
        guard let relay = value("CoderPushRelayURL"), let gateway = value("CoderPushGatewayURL"),
              let profile = value("CoderPushAppProfile") else { return nil }
        return PushSettings(relay: relay, gateway: gateway, profile: profile)
    }
}

/// Hands each APNs token to Rust, holding one that arrives before the
/// reader is ready.
@MainActor
final class PushRegistration {
    static let shared = PushRegistration()
    private var deliver: ((String) -> Void)?
    private var report: ((String) -> Void)?
    private var pending: String?

    /// Ask for permission and a token once per launch, only when push is
    /// configured. Synthetic launches never register; a loopback test
    /// launch does, so a test can check the handoff to Rust.
    func start(synthetic: Bool, deliver: @escaping (String) -> Void, failed: @escaping (String) -> Void) {
        guard !synthetic, PushSettings.configured != nil, self.deliver == nil else { return }
        self.deliver = deliver
        report = failed
        if let pending { self.pending = nil; deliver(pending) }
        UNUserNotificationCenter.current().requestAuthorization(options: [.alert, .sound]) { granted, _ in
            Task { @MainActor in
                if granted {
                    UIApplication.shared.registerForRemoteNotifications()
                } else {
                    PushRegistration.shared.report?(
                        "Notifications are off for Coder. Turn them on in Settings to get wakes.")
                }
            }
        }
    }

    func received(_ token: Data) {
        let hex = token.map { String(format: "%02x", $0) }.joined()
        if let deliver { deliver(hex) } else { pending = hex }
    }

    func failed(_ error: Error) {
        // Without the aps-environment entitlement, registration fails here.
        report?("Couldn't register for wakes: \(error.localizedDescription)")
    }
}

/// The system reports APNs registration only to the application delegate.
final class PushAppDelegate: NSObject, UIApplicationDelegate {
    func application(_ application: UIApplication,
                     didRegisterForRemoteNotificationsWithDeviceToken deviceToken: Data) {
        Task { @MainActor in PushRegistration.shared.received(deviceToken) }
    }

    func application(_ application: UIApplication,
                     didFailToRegisterForRemoteNotificationsWithError error: Error) {
        Task { @MainActor in PushRegistration.shared.failed(error) }
    }
}

// Only an open terminal may rotate; see TerminalOrientation.
extension PushAppDelegate {
    func application(_ application: UIApplication,
                     supportedInterfaceOrientationsFor window: UIWindow?) -> UIInterfaceOrientationMask {
        TerminalOrientation.mask
    }
}
