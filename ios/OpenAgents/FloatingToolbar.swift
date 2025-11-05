import SwiftUI
#if os(iOS)
import UIKit
#endif

/// A small floating toolbar anchored above the bottom-right corner on iPhone.
/// - Uses Liquid Glass on supported OS versions; falls back to thin material.
struct FloatingToolbar: View {
    var body: some View {
        #if os(iOS)
        Group {
            if UIDevice.current.userInterfaceIdiom == .phone {
                content
            } else {
                EmptyView()
            }
        }
        #else
        EmptyView()
        #endif
    }

    private var content: some View {
        // Foreground content (intrinsic size)
        let fg = HStack(spacing: 0) {
            // New message pencil temporarily disabled
            /*
            Button(action: {}, label: {
                Image(systemName: "pencil")
                    .renderingMode(.template)
                    .symbolRenderingMode(.monochrome)
                    .font(.system(size: 20, weight: .bold))
                    .foregroundStyle(Color.white)
                    .shadow(color: Color.black.opacity(0.25), radius: 0.5, x: 0, y: 0)
                    .accessibilityLabel("New message")
                    .frame(width: 36, height: 36)
            })
            .buttonStyle(.plain)
            */
        }
        .padding(.horizontal, 8)
        .padding(.vertical, 8)

        // Apply background sized to the foreground, so it never expands to full screen
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
            .shadow(color: Color.black.opacity(0.35), radius: 12, x: 0, y: 8)
            .padding(.trailing, 14)
            .padding(.bottom, 18)
    }
}

#Preview {
    ZStack(alignment: .bottomTrailing) {
        OATheme.Colors.background.ignoresSafeArea()
        FloatingToolbar()
    }
}
