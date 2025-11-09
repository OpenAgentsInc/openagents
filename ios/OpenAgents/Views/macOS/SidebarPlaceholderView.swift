import SwiftUI

#if os(macOS)
struct SidebarPlaceholderView: View {
    var body: some View {
        List {
            Section("Sidebar") {
                Text("Session history will appear here.")
                    .font(OAFonts.ui(.subheadline, 12))
                    .foregroundStyle(OATheme.Colors.textSecondary)
            }
        }
        .listStyle(.sidebar)
        .scrollContentBackground(.hidden)
        .background {
            if #available(macOS 15.0, *) {
                Rectangle()
                    .fill(.clear)
                    .glassEffect(.regular, in: Rectangle())
                    .ignoresSafeArea(.container, edges: .top)
            } else {
                OATheme.Colors.sidebarBackground
            }
        }
        .overlay(alignment: .trailing) {
            Rectangle()
                .fill(OATheme.Colors.background)
                .frame(width: 2)
        }
    }

    // No extra material; rely on OATheme surfaces
}
#endif
