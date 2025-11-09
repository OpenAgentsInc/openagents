import SwiftUI

#if os(macOS)
struct ChatMacOSView: View {
    @State private var columnVisibility: NavigationSplitViewVisibility = .all
    @State private var showInspector: Bool = false
    private let defaultSidebarWidth: CGFloat = 260

    var body: some View {
        NavigationSplitView(columnVisibility: $columnVisibility) {
            SessionSidebarView()
                .navigationSplitViewColumnWidth(min: defaultSidebarWidth, ideal: defaultSidebarWidth, max: defaultSidebarWidth)
        } detail: {
            ChatAreaView()
                .navigationSplitViewColumnWidth(min: 800, ideal: 1200)
                .navigationTitle("")
        }
        .navigationSplitViewStyle(.balanced)
    }

    @ViewBuilder
    private var splitView: some View { EmptyView() }

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
