import SwiftUI
#if os(iOS)
import UIKit
#endif

/// Two floating scroll control buttons (up/down) stacked above the compose button.
struct FloatingScrollButtons: View {
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
        VStack(spacing: 10) {
            glassButton(system: "chevron.up", label: "Scroll to top") {
                NotificationCenter.default.post(name: .acpScrollToTop, object: nil)
            }
            glassButton(system: "chevron.down", label: "Scroll to bottom") {
                NotificationCenter.default.post(name: .acpScrollToBottom, object: nil)
            }
        }
        .padding(.trailing, 14)
        .padding(.bottom, 76) // sit above the compose pencil
    }

    private func glassButton(system: String, label: String, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            Image(systemName: system)
                .renderingMode(.template)
                .symbolRenderingMode(.monochrome)
                .font(.system(size: 18, weight: .bold))
                .foregroundStyle(Color.white)
                .frame(width: 36, height: 36)
                .accessibilityLabel(label)
        }
        .buttonStyle(.plain)
        .background(
            Group {
                #if os(iOS)
                if #available(iOS 26, *) {
                    GlassEffectContainer {
                        Circle()
                            .fill(Color.clear)
                            .glassEffect(.regular, in: Circle())
                    }
                } else {
                    Circle().fill(.ultraThinMaterial)
                }
                #else
                Circle().fill(.regularMaterial)
                #endif
            }
        )
        .background(
            Circle().fill(LinearGradient(colors: [Color.black.opacity(0.16), Color.black.opacity(0.06)], startPoint: .top, endPoint: .bottom))
        )
        .overlay(
            Circle().strokeBorder(OATheme.Colors.border.opacity(0.6), lineWidth: 1)
        )
        .clipShape(Circle())
        .shadow(color: Color.black.opacity(0.35), radius: 12, x: 0, y: 8)
    }
}

extension Notification.Name {
    static let acpScrollToTop = Notification.Name("AcpThreadViewScrollToTop")
    static let acpScrollToBottom = Notification.Name("AcpThreadViewScrollToBottom")
}

#Preview {
    ZStack(alignment: .bottomTrailing) {
        OATheme.Colors.background.ignoresSafeArea()
        FloatingScrollButtons()
    }
}

