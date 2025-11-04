import SwiftUI

struct GlassTerminalCard: View {
    var title: String = "openagents — bash"
    var lines: [String] = [
        "$ cargo bridge",
        "Listening on 0.0.0.0:8787 …",
        "Client connected — ws://localhost:8787/ws",
        "tinyvex.update threads: upsert",
    ]

    var body: some View {
        Group {
            if #available(iOS 26, macOS 15, *) {
                GlassEffectContainer {
                    card
                        .glassEffectID("terminal-card")
                }
            } else {
                card
            }
        }
        .padding(16)
    }

    private var card: some View {
        ZStack(alignment: .topLeading) {
            // Glass background (Liquid Glass on 26+, thin material fallback pre‑26)
            Group {
                if #available(iOS 26, macOS 15, *) {
                    RoundedRectangle(cornerRadius: 16, style: .continuous)
                        .fill(Color.clear)
                        .glassEffect(.regular, in: RoundedRectangle(cornerRadius: 16, style: .continuous))
                        .overlay(
                            // Dark tint to fit our offblack scheme
                            RoundedRectangle(cornerRadius: 16, style: .continuous)
                                .fill(LinearGradient(colors: [Color.black.opacity(0.35), Color.black.opacity(0.15)], startPoint: .top, endPoint: .bottom))
                        )
                } else {
                    RoundedRectangle(cornerRadius: 16, style: .continuous)
                        .fill(.ultraThinMaterial)
                        .overlay(
                            RoundedRectangle(cornerRadius: 16, style: .continuous)
                                .fill(LinearGradient(colors: [Color.black.opacity(0.35), Color.black.opacity(0.15)], startPoint: .top, endPoint: .bottom))
                        )
                }
            }
            .overlay(
                RoundedRectangle(cornerRadius: 16, style: .continuous)
                    .strokeBorder(OATheme.Colors.border.opacity(0.6), lineWidth: 1)
            )

            VStack(alignment: .leading, spacing: 10) {
                HStack(spacing: 8) {
                    Image(systemName: "terminal.fill")
                        .imageScale(.small)
                        .foregroundStyle(OATheme.Colors.textSecondary)
                    Text(title)
                        .font(Font.custom(BerkeleyFont.defaultName(), size: 13, relativeTo: .caption))
                        .foregroundStyle(OATheme.Colors.textSecondary)
                }
                .padding(.bottom, 2)

                ForEach(Array(lines.enumerated()), id: \.offset) { _, line in
                    Text(line)
                        .font(Font.custom(BerkeleyFont.defaultName(), size: 14, relativeTo: .body))
                        .foregroundStyle(OATheme.Colors.textPrimary)
                        .textSelection(.enabled)
                }
            }
            .padding(16)
        }
        .shadow(color: Color.black.opacity(0.35), radius: 12, x: 0, y: 8)
    }
}

#Preview {
    ZStack {
        OATheme.Colors.background.ignoresSafeArea()
        GlassTerminalCard()
    }
}

