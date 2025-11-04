import SwiftUI

struct GlassHeader: View {
    var title: String
    var body: some View {
        // Fallback glassy header using system material so we build across SDKs
        ZStack(alignment: .leading) {
            Rectangle().fill(.ultraThinMaterial)
            HStack(spacing: 8) {
                Image(systemName: "sparkles")
                    .imageScale(.medium)
                    .foregroundStyle(OATheme.Colors.textSecondary)
                Text(title)
                    .font(.subheadline)
                    .foregroundStyle(OATheme.Colors.textPrimary)
            }
            .padding(.horizontal, 12)
        }
        .frame(height: 44)
        .overlay(Divider().opacity(0.25), alignment: .bottom)
        .ignoresSafeArea(edges: .top)
    }
}
