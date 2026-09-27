// The world is the app's home. Rust owns interaction and reader authority.
import SwiftUI

@main
struct CoderApp: App {
    @UIApplicationDelegateAdaptor(PushAppDelegate.self) private var pushDelegate
    @StateObject private var reader = MobileBridge(
        synthetic: ProcessInfo.processInfo.arguments.contains("--synthetic"),
        loopbackTest: ProcessInfo.processInfo.arguments.contains("--loopback-test"))

    var body: some Scene {
        WindowGroup {
            VerseScreen(reader: reader,
                        synthetic: ProcessInfo.processInfo.arguments.contains("--synthetic"))
                .tint(Color(red: 1, green: 176.0 / 255.0, blue: 0))
                .task {
                    // Does nothing unless this build is configured for push.
                    let arguments = ProcessInfo.processInfo.arguments
                    PushRegistration.shared.start(
                        synthetic: arguments.contains("--synthetic") && !arguments.contains("--loopback-test"),
                        deliver: { reader.pushToken($0) }, failed: { reader.pushFailed($0) })
                }
        }
    }
}
