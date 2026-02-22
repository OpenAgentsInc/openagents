import SwiftUI

struct ContentView: View {
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

#Preview {
    ContentView()
}
