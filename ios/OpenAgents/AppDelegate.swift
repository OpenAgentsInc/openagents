import Foundation
#if os(iOS)
import UIKit

final class AppDelegate: NSObject, UIApplicationDelegate {
    func application(_ application: UIApplication, willFinishLaunchingWithOptions launchOptions: [UIApplication.LaunchOptionsKey : Any]? = nil) -> Bool {
        print("[Bridge][app] willFinishLaunching at \(ISO8601DateFormatter().string(from: Date()))")
        return true
    }

    func application(_ application: UIApplication, didFinishLaunchingWithOptions launchOptions: [UIApplication.LaunchOptionsKey : Any]? = nil) -> Bool {
        print("[Bridge][app] didFinishLaunching at \(ISO8601DateFormatter().string(from: Date()))")
        return true
    }

    func applicationDidBecomeActive(_ application: UIApplication) {
        print("[Bridge][app] applicationDidBecomeActive at \(ISO8601DateFormatter().string(from: Date()))")
    }

    func applicationWillEnterForeground(_ application: UIApplication) {
        print("[Bridge][app] applicationWillEnterForeground at \(ISO8601DateFormatter().string(from: Date()))")
    }
}
#endif

