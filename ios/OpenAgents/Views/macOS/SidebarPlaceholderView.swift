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
        // Mask the system split divider with our theme color (make it thicker to cover glow)
        .overlay(alignment: .trailing) {
            Rectangle()
                .fill(OATheme.Colors.background)
                .frame(width: 12)
        }
    }

    // No extra material; rely on OATheme surfaces
}
#endif
