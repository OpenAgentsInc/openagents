import SwiftUI

#if os(macOS)
struct ChatMacOSView: View {
    @State private var columnVisibility: NavigationSplitViewVisibility = .all
    @State private var showInspector: Bool = false

    var body: some View {
        Group {
            splitView
        }
        // Ensure our palette is the base surface
        .background(OATheme.Colors.background.ignoresSafeArea())
    }

    @ViewBuilder
    private var splitView: some View {
        if showInspector {
            AnyView(
                NavigationSplitView(columnVisibility: $columnVisibility) {
                    SidebarPlaceholderView()
                        .navigationSplitViewColumnWidth(min: 220, ideal: 250, max: 280)
                } content: {
                    ChatAreaPlaceholderView()
                        .navigationSplitViewColumnWidth(min: 600, ideal: 900)
                } detail: {
                    InspectorPlaceholderView()
                        .navigationSplitViewColumnWidth(min: 280, ideal: 320, max: 360)
                }
                .navigationSplitViewStyle(.balanced)
            )
        } else {
            AnyView(
                NavigationSplitView {
                    SidebarPlaceholderView()
                        .navigationSplitViewColumnWidth(min: 220, ideal: 250, max: 300)
                } detail: {
                    ChatAreaPlaceholderView()
                        .navigationSplitViewColumnWidth(min: 800, ideal: 1200)
                }
                .navigationSplitViewStyle(.balanced)
            )
        }
    }

    private func toggleSidebar() {
        switch columnVisibility {
        case .all:
            columnVisibility = .detailOnly
        default:
            columnVisibility = .all
        }
    }
}
#endif
