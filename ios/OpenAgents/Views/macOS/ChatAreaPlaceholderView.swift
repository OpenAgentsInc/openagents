import SwiftUI

#if os(macOS)
struct ChatAreaPlaceholderView: View {
    var body: some View {
        ScrollView {
            VStack(spacing: 16) {
                Image(systemName: "bubble.left.and.bubble.right")
                    .font(.system(size: 48, weight: .regular))
                    .foregroundStyle(OATheme.Colors.textSecondary)
                Text("Chat Area Placeholder")
                    .font(OAFonts.ui(.headline, 16))
                    .foregroundStyle(OATheme.Colors.textPrimary)
                Text("Messages will render here using ACP components.")
                    .font(OAFonts.ui(.subheadline, 12))
                    .foregroundStyle(OATheme.Colors.textSecondary)
            }
            .frame(maxWidth: .infinity)
            .padding(32)
        }
        .background(OATheme.Colors.background)
        // Hard scroll edge effect (optional visual polish)
    }

    // No extra material; rely on OATheme surfaces
}
#endif
