// Rust builds the screen; this host decodes and renders it.
import SwiftUI

@main
struct OpenAgentsApp: App {
    var body: some Scene {
        WindowGroup {
            HomeScreen()
        }
    }
}

struct HomeScreen: View {
    private let view: NativeView? = {
        let buffer = openagents_mobile_home()
        defer { openagents_mobile_buffer_free(buffer) }
        guard let data = buffer.data, buffer.len > 0 else { return nil }
        return try? JSONDecoder().decode(NativeView.self, from: Data(bytes: data, count: buffer.len))
    }()

    var body: some View {
        ZStack {
            Color.black.ignoresSafeArea()
            if let view {
                NativeRenderer(node: view.root, revision: view.revision, followTarget: nil,
                               followChanged: nil, activate: { _ in })
                    .fixedSize(horizontal: true, vertical: false)
            } else {
                Text("OpenAgents could not load this screen.").foregroundStyle(.white)
            }
        }
    }
}
