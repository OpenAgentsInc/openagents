import SwiftUI

#if os(macOS)
struct ChatMacOSView: View {
    @State private var columnVisibility: NavigationSplitViewVisibility = .all
    @State private var showInspector: Bool = true

    var body: some View {
        Group {
            splitView
        }
        // Ensure our palette is the base surface
        .background(OATheme.Colors.background.ignoresSafeArea())
    }

    @ViewBuilder
    private var splitView: some View {
        NavigationSplitView(columnVisibility: $columnVisibility) {
            SidebarPlaceholderView()
                .navigationSplitViewColumnWidth(min: 220, ideal: 250, max: 280)
        } content: {
            ChatAreaPlaceholderView()
                .navigationSplitViewColumnWidth(min: 400, ideal: 600)
        } detail: {
            if showInspector {
                InspectorPlaceholderView()
                    .navigationSplitViewColumnWidth(min: 280, ideal: 300, max: 350)
            }
        }
        .navigationSplitViewStyle(.balanced)
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
