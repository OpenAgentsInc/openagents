// Firm reason: 6 (iOS root container hosting the full-screen Coax/Metal GPU surface with pure-black native modal sheets for Computers, Workspace, Subagent, Antigravity Hub, and Side Question).
import SwiftUI

struct RootView: View {
    @ObservedObject var store: AppStore

    var body: some View {
        GpuRootHostView(store: store)
            .ignoresSafeArea()
            .background(Color.black.ignoresSafeArea())
            .sheet(isPresented: $store.isComputersPresented) {
                ComputersView(store: store)
                    .preferredColorScheme(.dark)
            }
            .sheet(isPresented: $store.isWorkspacePresented) {
                WorkspaceView(store: store)
                    .preferredColorScheme(.dark)
            }
            .sheet(item: $store.activeSubagentItem) { sub in
                SubagentSheetView(store: store, subagent: sub)
                    .preferredColorScheme(.dark)
            }
            .sheet(isPresented: $store.isHubPresented) {
                AntigravityHubView(store: store)
                    .preferredColorScheme(.dark)
            }
            .sheet(isPresented: $store.isSideQuestionPresented) {
                SideQuestionSheetView(store: store)
                    .preferredColorScheme(.dark)
            }
            .sheet(isPresented: $store.isUsagePresented) {
                UsageDashboardView(store: store)
                    .preferredColorScheme(.dark)
            }
    }
}
