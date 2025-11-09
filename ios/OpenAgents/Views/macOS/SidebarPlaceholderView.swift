import SwiftUI

#if os(macOS)
struct SidebarPlaceholderView: View {
    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 8) {
                Text("Sidebar Placeholder")
                    .font(OAFonts.ui(.headline, 14))
                Text("Session history will appear here.")
                    .font(OAFonts.ui(.subheadline, 12))
                    .foregroundStyle(OATheme.Colors.textSecondary)
            }
            .padding()
        }
        .background(OATheme.Colors.sidebarBackground)
        // Hard scroll edge effect (optional visual polish)
    }

    // No extra material; rely on OATheme surfaces
}
#endif
