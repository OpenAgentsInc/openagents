import SwiftUI

#if os(macOS)
struct InspectorPlaceholderView: View {
    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 8) {
                Text("Inspector Placeholder")
                    .font(OAFonts.ui(.headline, 14))
                Text("Tool details / JSON output will appear here.")
                    .font(OAFonts.ui(.subheadline, 12))
                    .foregroundStyle(OATheme.Colors.textSecondary)
            }
            .padding()
        }
        .background(backgroundMaterial)
        // Apply hard scroll edge effect on macOS 15+ when available
        // (older SDKs/Xcode may not expose scrollEdgeEffect API)
        .modifier(ScrollEdgeHardIfAvailable())
    }

    @ViewBuilder
    private var backgroundMaterial: some View {
        if #available(macOS 15.0, *) {
            Rectangle()
                .fill(.clear)
                .glassEffect(.regular, in: Rectangle())
        } else {
            Color.clear
                .background(.ultraThinMaterial)
        }
    }
}

private struct ScrollEdgeHardIfAvailable: ViewModifier {
    func body(content: Content) -> some View {
        if #available(macOS 15.0, *) {
            // Use new scroll edge effect API when available
            return AnyView(content.scrollEdgeEffect(.hard))
        } else {
            return AnyView(content)
        }
    }
}
#endif
