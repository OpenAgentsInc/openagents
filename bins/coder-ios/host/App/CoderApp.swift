// The world is the app's home. Rust owns interaction and reader authority.
import SwiftUI

@main
struct CoderApp: App {
    @StateObject private var reader = MobileBridge(
        synthetic: ProcessInfo.processInfo.arguments.contains("--synthetic"))

    var body: some Scene {
        WindowGroup {
            VerseScreen(reader: reader,
                        synthetic: ProcessInfo.processInfo.arguments.contains("--synthetic"))
                .tint(Color(red: 1, green: 176.0 / 255.0, blue: 0))
        }
    }
}
