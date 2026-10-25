import UIKit

// Started from main.swift (SceneStateMigration runs first).
final class AppDelegate: UIResponder, UIApplicationDelegate {
    func application(
        _ application: UIApplication,
        didFinishLaunchingWithOptions launchOptions: [UIApplication.LaunchOptionsKey: Any]?
    ) -> Bool {
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

    func application(
        _ application: UIApplication,
        configurationForConnecting connectingSceneSession: UISceneSession,
        options: UIScene.ConnectionOptions
    ) -> UISceneConfiguration {
        Self.windowConfiguration(for: connectingSceneSession)
    }

    /// Grid-only is the app's starting point; there is no chat startup path.
    static let sceneDelegateClass: AnyClass = GridOnlySceneDelegate.self

    static func windowConfiguration(for session: UISceneSession) -> UISceneConfiguration {
        // Do not inherit the previous SwiftUI app's archived delegate.
        let configuration = UISceneConfiguration(name: "Coder", sessionRole: session.role)
        configuration.sceneClass = UIWindowScene.self
        configuration.delegateClass = sceneDelegateClass
        configuration.storyboard = nil
        return configuration
    }
}
