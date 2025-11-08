import Foundation
#if os(iOS)
import UIKit
import OSLog
import OpenAgentsCore

final class AppDelegate: NSObject, UIApplicationDelegate {
    func application(_ application: UIApplication, willFinishLaunchingWithOptions launchOptions: [UIApplication.LaunchOptionsKey : Any]? = nil) -> Bool {
        let ts = ISO8601DateFormatter().string(from: Date())
        OpenAgentsLog.app.info("willFinishLaunching at \(ts)")
        return true
    }

    func application(_ application: UIApplication, didFinishLaunchingWithOptions launchOptions: [UIApplication.LaunchOptionsKey : Any]? = nil) -> Bool {
        let ts = ISO8601DateFormatter().string(from: Date())
        OpenAgentsLog.app.info("didFinishLaunching at \(ts)")
        // Prewarm fonts and haptics early to avoid first-focus lag when composer opens
        _ = BerkeleyFont.registerAll()
        PerformanceWarmup.preloadMonoFont()
        PerformanceWarmup.prewarmHaptics()
        PerformanceWarmup.prewarmResponderSilently()
        return true
    }

    func applicationDidBecomeActive(_ application: UIApplication) {
        let ts = ISO8601DateFormatter().string(from: Date())
        OpenAgentsLog.app.info("applicationDidBecomeActive at \(ts)")
        // Ensure keyboard/text input is also prewarmed once we have a key window
        PerformanceWarmup.prewarmKeyboardAndTextInput()
    }

    func applicationWillEnterForeground(_ application: UIApplication) {
        let ts = ISO8601DateFormatter().string(from: Date())
        OpenAgentsLog.app.info("applicationWillEnterForeground at \(ts)")
    }
}
#endif
