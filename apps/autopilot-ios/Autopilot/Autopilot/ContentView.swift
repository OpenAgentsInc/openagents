import SwiftUI

/// Production iOS Codex root: WGPUI renderer surface only.
struct WgpuiCodexRootView: View {
    @StateObject private var model = CodexHandshakeViewModel()
    @Environment(\.scenePhase) private var scenePhase

    var body: some View {
        WgpuiBackgroundView(model: model)
            .task {
                await model.autoConnectOnLaunch()
            }
            .onAppear {
                model.handleScenePhaseChange(scenePhase)
            }
            .onChange(of: scenePhase) { _, newPhase in
                model.handleScenePhaseChange(newPhase)
            }
            .preferredColorScheme(.dark)
    }
}

@available(*, deprecated, message: "Use WgpuiCodexRootView for production UI.")
struct ContentView: View {
    var body: some View {
        WgpuiCodexRootView()
    }
}

#Preview {
    WgpuiCodexRootView()
}
