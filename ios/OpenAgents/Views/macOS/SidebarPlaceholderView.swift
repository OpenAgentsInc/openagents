import SwiftUI

#if os(macOS)
struct SidebarPlaceholderView: View {
    var body: some View {
        ZStack(alignment: .leading) {
            // Solid theme surface to avoid system vibrancy edges
            OATheme.Colors.sidebarBackground

            // Content (no scroll effects in placeholder)
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
            .scrollDisabled(true)

            // Leading edge normalize (in case of shadow/glow)
            Rectangle()
                .fill(OATheme.Colors.sidebarBackground)
                .frame(width: 8)
                .allowsHitTesting(false)

            // Trailing edge: cover NSSplitView divider completely
            Rectangle()
                .fill(OATheme.Colors.background)
                .frame(width: 16)
                .frame(maxWidth: .infinity, alignment: .trailing)
                .allowsHitTesting(false)
        }
        .clipped()
    }

    // No extra material; rely on OATheme surfaces
}
#endif
