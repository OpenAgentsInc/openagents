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
        .scrollEdgeEffect(.hard)
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
#endif
