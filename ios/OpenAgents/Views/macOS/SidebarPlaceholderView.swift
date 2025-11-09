import SwiftUI

#if os(macOS)
struct SidebarPlaceholderView: View {
    var body: some View {
        ZStack(alignment: .leading) {
            // Base surface (fallback)
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

            // Apply Liquid Glass when available (under content overlays)
            if #available(macOS 15.0, *) {
                Rectangle()
                    .fill(.clear)
                    .glassEffect(.regular, in: Rectangle())
                    .ignoresSafeArea(.container, edges: .top)
                    .allowsHitTesting(false)
            }

            // Trailing edge: cover NSSplitView divider completely
            Rectangle()
                .fill(OATheme.Colors.background)
                .frame(width: 2)
                .frame(maxWidth: .infinity, alignment: .trailing)
                .allowsHitTesting(false)
            // Top overlay to unify tone under the window toolbar
            Rectangle()
                .fill(OATheme.Colors.sidebarBackground)
                .frame(height: 32)
                .frame(maxHeight: .infinity, alignment: .top)
                .allowsHitTesting(false)
        }
        .clipped()
        .ignoresSafeArea(.container, edges: .top)
    }

    // No extra material; rely on OATheme surfaces
}
#endif
