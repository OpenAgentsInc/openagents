import SwiftUI

#if os(iOS)
/// Reusable top header that you can attach to any screen inside a NavigationStack.
/// iOS 26+ only: uses Liquid Glass button styles directly.
struct ChatHeaderToolbar: ToolbarContent {
    @Environment(\.colorScheme) private var colorScheme
    var title: String
    var onToggleMenu: () -> Void
    var onNewChat: () -> Void

    var body: some ToolbarContent {
        // LEFT: Navigation menu toggle (hamburger)
        ToolbarItem(placement: .topBarLeading) {
            Button(action: onToggleMenu) {
                Image(systemName: "line.3.horizontal")
                    .accessibilityLabel("Open navigation menu")
                    .foregroundStyle(colorScheme == .dark ? Color.white : OATheme.Colors.textPrimary)
            }
            .tint(colorScheme == .dark ? .white : OATheme.Colors.textPrimary)
            // Rely on the system’s toolbar chrome for Liquid Glass.
            // Avoid explicit .glass here to prevent the inner oval effect.
        }

        // CENTER: Title, left-of-center inside the principal slot
        ToolbarItem(placement: .principal) {
            // The principal slot is centered by the system; we left-align inside it
            // to achieve the requested "left-of-center" feel.
            HStack(spacing: 0) {
                Text(title)
                    .font(OAFonts.ui(.headline, 16))
                    .lineLimit(1)
                    .minimumScaleFactor(0.9)
                    .truncationMode(.tail)
                Spacer(minLength: 0)
            }
            .frame(maxWidth: .infinity, alignment: .leading)
        }

        // RIGHT: New Chat action (commented out for now)
        // ToolbarItem(placement: .topBarTrailing) {
        //     Button {
        //         onNewChat()
        //     } label: {
        //         Label("New Chat", systemImage: "plus.bubble")
        //             .labelStyle(.titleAndIcon)
        //     }
        //     .accessibilityLabel("Start new chat")
        //     .keyboardShortcut("n")
        //     .buttonStyle(.glassProminent)
        // }
    }
}
#endif
