// Rust builds the screen; this host decodes and renders it.
import SwiftUI

@main
struct OpenAgentsApp: App {
    @StateObject private var bridge = MobileBridge()
    @Environment(\.scenePhase) private var scenePhase

    var body: some Scene {
        WindowGroup {
            HomeScreen(bridge: bridge)
                .onChange(of: scenePhase, initial: true) { _, phase in
                    if phase == .active { bridge.refresh() }
                }
        }
    }
}

struct HomeScreen: View {
    @ObservedObject var bridge: MobileBridge

    var body: some View {
        ZStack(alignment: .topTrailing) {
            Color.black.ignoresSafeArea()
            if let view = bridge.view {
                NativeRenderer(node: view.root, revision: view.revision, followTarget: nil,
                               followChanged: nil, activate: bridge.activate)
                    .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
            }
            if bridge.busy {
                ProgressView().tint(.white).padding()
                    .accessibilityLabel("Checking your tailnet")
            }
        }
        .tint(.white)
    }
}
