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
        Group {
            if #available(iOS 26, *) {
                GlassEffectContainer {
                    HStack(spacing: 0) {
                        Button(action: {}, label: {
                            Image(systemName: "pencil")
                                .symbolRenderingMode(.monochrome)
                                .font(.system(size: 20, weight: .bold))
                                .foregroundStyle(OATheme.Colors.textPrimary)
                                .accessibilityLabel("New message")
                                .frame(width: 36, height: 36)
                        })
                        .buttonStyle(.plain)
                    }
                    .padding(.horizontal, 8)
                    .padding(.vertical, 8)
                    .background(
                        ZStack {
                            Capsule(style: .continuous)
                                .fill(Color.clear)
                                .glassEffect(.regular, in: Capsule(style: .continuous))
                            // Subtle dark tint to fit our offblack theme
                            Capsule(style: .continuous)
                                .fill(LinearGradient(colors: [Color.black.opacity(0.35), Color.black.opacity(0.15)], startPoint: .top, endPoint: .bottom))
                        }
                    )
                    .overlay(
                        Capsule(style: .continuous)
                            .strokeBorder(OATheme.Colors.border.opacity(0.6), lineWidth: 1)
                    )
                    .shadow(color: Color.black.opacity(0.35), radius: 12, x: 0, y: 8)
                    .tint(.primary)
                }
            } else {
                // Fallback for earlier iOS versions without Liquid Glass
                HStack(spacing: 0) {
                    Button(action: {}, label: {
                        Image(systemName: "pencil")
                            .symbolRenderingMode(.monochrome)
                            .font(.system(size: 20, weight: .bold))
                            .foregroundStyle(OATheme.Colors.textPrimary)
                            .accessibilityLabel("New message")
                            .frame(width: 36, height: 36)
                    })
                    .buttonStyle(.plain)
                }
                .padding(.horizontal, 8)
                .padding(.vertical, 8)
                .background(
                    Capsule(style: .continuous)
                        .fill(.ultraThinMaterial)
                        .overlay(
                            Capsule(style: .continuous)
                                .fill(LinearGradient(colors: [Color.black.opacity(0.35), Color.black.opacity(0.15)], startPoint: .top, endPoint: .bottom))
                        )
                )
                .overlay(
                    Capsule(style: .continuous)
                        .strokeBorder(OATheme.Colors.border.opacity(0.6), lineWidth: 1)
                )
                .shadow(color: Color.black.opacity(0.35), radius: 12, x: 0, y: 8)
                .tint(.primary)
            }
        }
        // Place a little above the home indicator; parent should use .overlay(alignment: .bottomTrailing)
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
