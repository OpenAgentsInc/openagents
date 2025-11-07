import Foundation
#if os(iOS)
import UIKit
import OSLog

final class AppDelegate: NSObject, UIApplicationDelegate {
    func application(_ application: UIApplication, willFinishLaunchingWithOptions launchOptions: [UIApplication.LaunchOptionsKey : Any]? = nil) -> Bool {
        let ts = ISO8601DateFormatter().string(from: Date())
        print("[Bridge][app] willFinishLaunching at \(ts)")
        if #available(iOS 16.0, *) { Logger(subsystem: "com.openagents.app", category: "app").log("willFinishLaunching at \(ts, privacy: .public)") }
        return true
    }

    func application(_ application: UIApplication, didFinishLaunchingWithOptions launchOptions: [UIApplication.LaunchOptionsKey : Any]? = nil) -> Bool {
        let ts = ISO8601DateFormatter().string(from: Date())
        print("[Bridge][app] didFinishLaunching at \(ts)")
        if #available(iOS 16.0, *) { Logger(subsystem: "com.openagents.app", category: "app").log("didFinishLaunching at \(ts, privacy: .public)") }
        // Prewarm fonts and haptics early to avoid first-focus lag when composer opens
        _ = BerkeleyFont.registerAll()
        PerformanceWarmup.preloadMonoFont()
        PerformanceWarmup.prewarmHaptics()
        return true
    }

    func applicationDidBecomeActive(_ application: UIApplication) {
        let ts = ISO8601DateFormatter().string(from: Date())
        print("[Bridge][app] applicationDidBecomeActive at \(ts)")
        if #available(iOS 16.0, *) { Logger(subsystem: "com.openagents.app", category: "app").log("applicationDidBecomeActive at \(ts, privacy: .public)") }
        // Ensure keyboard/text input is also prewarmed once we have a key window
        PerformanceWarmup.prewarmKeyboardAndTextInput()
    }

    func applicationWillEnterForeground(_ application: UIApplication) {
        let ts = ISO8601DateFormatter().string(from: Date())
        print("[Bridge][app] applicationWillEnterForeground at \(ts)")
        if #available(iOS 16.0, *) { Logger(subsystem: "com.openagents.app", category: "app").log("applicationWillEnterForeground at \(ts, privacy: .public)") }
    }
}
#endif
