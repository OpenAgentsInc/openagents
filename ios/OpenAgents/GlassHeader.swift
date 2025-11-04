import SwiftUI

struct GlassHeader: View {
    var title: String
    var body: some View {
        Group {
            // Preferred: first‑party Liquid Glass
            #if canImport(SwiftUI)
            if #available(iOS 26, macOS 15, *) {
                GlassEffectContainer {
                    ZStack(alignment: .leading) {
                        Rectangle()
                            .fill(Color.clear)
                            .glassEffect(.regular, in: Rectangle())
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
                }
                .frame(height: 44)
                .overlay(Divider().opacity(0.25), alignment: .bottom)
            } else {
                // Fallback for earlier OS/SDKs
                fallbackHeader
            }
            #else
            fallbackHeader
            #endif
        }
        .ignoresSafeArea(edges: .top)
    }

    private var fallbackHeader: some View {
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
    }
}
