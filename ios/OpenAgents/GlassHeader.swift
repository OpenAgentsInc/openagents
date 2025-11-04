import SwiftUI

struct GlassHeader: View {
    var title: String
    var body: some View {
        Group {
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
                ZStack(alignment: .leading) {
                    Rectangle().fill(.ultraThinMaterial)
                    HStack(spacing: 8) {
                        Image(systemName: "sparkles").imageScale(.medium)
                        Text(title).font(.subheadline)
                    }
                    .padding(.horizontal, 12)
                }
                .frame(height: 44)
            }
        }
        .ignoresSafeArea(edges: .top)
    }
}

