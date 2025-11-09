import SwiftUI

#if os(macOS)
struct ChatMacOSView: View {
    @State private var columnVisibility: NavigationSplitViewVisibility = .all
    @State private var showInspector: Bool = true

    var body: some View {
        Group {
            if #available(macOS 15.0, *) {
                GlassEffectContainer {
                    splitView
                }
            } else {
                splitView
            }
        }
        .toolbar {
            ToolbarItem(placement: .primaryAction) {
                Button {
                    toggleSidebar()
                } label: {
                    Label("Toggle Sidebar", systemImage: "sidebar.left")
                }
                .keyboardShortcut("b", modifiers: .command)
            }
            ToolbarItem(placement: .primaryAction) {
                Button {
                    showInspector.toggle()
                } label: {
                    Label("Toggle Inspector", systemImage: "sidebar.right")
                }
                .keyboardShortcut("i", modifiers: .command)
            }
        }
    }

    @ViewBuilder
    private var splitView: some View {
        NavigationSplitView(columnVisibility: $columnVisibility) {
            SidebarPlaceholderView()
                .navigationSplitViewColumnWidth(min: 220, ideal: 250, max: 280)
                .modifier(GlassTag("sidebar"))
        } content: {
            ChatAreaPlaceholderView()
                .navigationSplitViewColumnWidth(min: 400, ideal: 600)
                .modifier(GlassTag("chat-area"))
        } detail: {
            if showInspector {
                InspectorPlaceholderView()
                    .navigationSplitViewColumnWidth(min: 280, ideal: 300, max: 350)
                    .modifier(GlassTag("inspector"))
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

private struct GlassTag: ViewModifier {
    let id: String
    init(_ id: String) { self.id = id }
    func body(content: Content) -> some View {
        if #available(macOS 15.0, *) {
            content.glassEffectID(id)
        } else {
            content
        }
    }
}
#endif

