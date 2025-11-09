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

            // Glass shape that "hovers" into the toolbar area
            if #available(macOS 15.0, *) {
                RoundedRectangle(cornerRadius: 12, style: .continuous)
                    .inset(by: 6)
                    .fill(.clear)
                    .glassEffect(.regular, in: RoundedRectangle(cornerRadius: 12, style: .continuous))
                    .offset(y: -14) // hover into toolbar
                    .ignoresSafeArea(.container, edges: .top)
                    .allowsHitTesting(false)
            }

            // Trailing edge: cover NSSplitView divider completely (over glass)
            Rectangle()
                .fill(OATheme.Colors.background)
                .frame(width: 2)
                .frame(maxWidth: .infinity, alignment: .trailing)
                .allowsHitTesting(false)
        }
        .clipped()
        .ignoresSafeArea(.container, edges: .top)
    }

    // No extra material; rely on OATheme surfaces
}
#endif
