import UIKit
import UserNotifications

// Started from main.swift (SceneStateMigration runs first).
final class AppDelegate: UIResponder, UIApplicationDelegate {
    func application(
        _ application: UIApplication,
        didFinishLaunchingWithOptions launchOptions: [UIApplication.LaunchOptionsKey: Any]?
    ) -> Bool {
        #if !GRID_ONLY
        // Before launch finishes, so a tap that launched the app is delivered.
        UNUserNotificationCenter.current().delegate = PushNotifications.shared
        #endif
        // Backstop: a session still bound to another app's delegate (kept by
        // the system despite the migration) is destroyed, so the next scene
        // comes from `windowConfiguration(for:)`.
        for session in application.openSessions
        where session.role == .windowApplication {
            guard let delegate = session.configuration.delegateClass,
                  ObjectIdentifier(delegate) != ObjectIdentifier(Self.sceneDelegateClass) else { continue }
            application.requestSceneSessionDestruction(session, options: nil)
        }
        return true
    }

    func application(_ application: UIApplication, didRegisterForRemoteNotificationsWithDeviceToken deviceToken: Data) {
        PushNotifications.shared.didRegister(deviceToken: deviceToken)
    }

    func application(_ application: UIApplication, didFailToRegisterForRemoteNotificationsWithError error: Error) {
        PushNotifications.shared.didFailToRegister(error)
    }

    func application(
        _ application: UIApplication,
        configurationForConnecting connectingSceneSession: UISceneSession,
        options: UIScene.ConnectionOptions
    ) -> UISceneConfiguration {
        Self.windowConfiguration(for: connectingSceneSession)
    }

    /// The Grid-only experiment (`GRID_ONLY`) opens on its own home screen.
    #if GRID_ONLY
    static let sceneDelegateClass: AnyClass = GridOnlySceneDelegate.self
    #else
    static let sceneDelegateClass: AnyClass = SceneDelegate.self
    #endif

    static func windowConfiguration(for session: UISceneSession) -> UISceneConfiguration {
        // Do not inherit the previous SwiftUI app's archived delegate.
        let configuration = UISceneConfiguration(name: "Coder", sessionRole: session.role)
        configuration.sceneClass = UIWindowScene.self
        configuration.delegateClass = sceneDelegateClass
        configuration.storyboard = nil
        return configuration
    }
}
