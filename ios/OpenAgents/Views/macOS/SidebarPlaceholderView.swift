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
        .background(OATheme.Colors.sidebarBackground.ignoresSafeArea(.container, edges: .top))
    }

    // No extra material; rely on OATheme surfaces
}
#endif
