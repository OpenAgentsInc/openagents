import SwiftUI
#if os(iOS)
import UIKit
#endif

/// Compact top-left navigation menu button styled with Liquid Glass.
struct FloatingMenuButton: View {
    var body: some View {
        #if os(iOS)
        Group {
            if UIDevice.current.userInterfaceIdiom == .phone {
                content
            } else { EmptyView() }
        }
        #else
        EmptyView()
        #endif
    }

    private var content: some View {
        let fg = HStack(spacing: 0) {
            Button(action: { /* TODO: open nav */ }, label: {
                Image(systemName: "line.3.horizontal")
                    .renderingMode(.template)
                    .symbolRenderingMode(.monochrome)
                    .font(.system(size: 18, weight: .semibold))
                    .foregroundStyle(Color.white)
                    .frame(width: 32, height: 32)
                    .accessibilityLabel("Navigation menu")
            })
            .buttonStyle(.plain)
        }
        .padding(.horizontal, 8)
        .padding(.vertical, 8)

        return fg
            .background(
                Group {
                    if #available(iOS 26, *) {
                        GlassEffectContainer {
                            Capsule(style: .continuous)
                                .fill(Color.clear)
                                .glassEffect(.clear, in: Capsule(style: .continuous))
                        }
                    } else {
                        Capsule(style: .continuous).fill(.ultraThinMaterial)
                    }
                }
            )
            .background(
                Capsule(style: .continuous)
                    .fill(LinearGradient(colors: [Color.black.opacity(0.16), Color.black.opacity(0.06)], startPoint: .top, endPoint: .bottom))
            )
            .overlay(
                Capsule(style: .continuous)
                    .strokeBorder(OATheme.Colors.border.opacity(0.6), lineWidth: 1)
            )
            .clipShape(Capsule(style: .continuous))
            .contentShape(Capsule(style: .continuous))
            .shadow(color: Color.black.opacity(0.25), radius: 8, x: 0, y: 4)
            .padding(.trailing, 14)
            .padding(.top, 14)
    }
}
