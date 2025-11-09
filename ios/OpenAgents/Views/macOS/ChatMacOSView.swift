import SwiftUI

#if os(macOS)
struct ChatMacOSView: View {
    @State private var columnVisibility: NavigationSplitViewVisibility = .all
    @State private var showInspector: Bool = false

    var body: some View {
        Group {
            if #available(macOS 15.0, *) {
                GlassEffectContainer {
                    NavigationSplitView(columnVisibility: $columnVisibility) {
                        SidebarPlaceholderView()
                            .navigationSplitViewColumnWidth(min: 220, ideal: 260, max: 300)
                    } detail: {
                        ChatAreaPlaceholderView()
                            .navigationSplitViewColumnWidth(min: 800, ideal: 1200)
                            .navigationTitle("")
                    }
                    .navigationSplitViewStyle(.balanced)
                }
            } else {
                NavigationSplitView(columnVisibility: $columnVisibility) {
                    SidebarPlaceholderView()
                        .navigationSplitViewColumnWidth(min: 220, ideal: 260, max: 300)
                } detail: {
                    ChatAreaPlaceholderView()
                        .navigationSplitViewColumnWidth(min: 800, ideal: 1200)
                        .navigationTitle("")
                }
                .navigationSplitViewStyle(.balanced)
            }
        }
        .background(OATheme.Colors.background.ignoresSafeArea())
        .toolbar(.visible, for: .windowToolbar)
        .toolbarBackground(OATheme.Colors.background, for: .windowToolbar)
        .toolbarBackgroundVisibility(.visible, for: .windowToolbar)
        .toolbarColorScheme(.dark, for: .windowToolbar)
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
