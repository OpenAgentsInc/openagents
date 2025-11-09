import SwiftUI

#if os(macOS)
struct ChatAreaPlaceholderView: View {
    var body: some View {
        ScrollView {
            VStack(spacing: 0) {
                Text("Hello")
                    .font(OAFonts.ui(.title, 48))
                    .foregroundStyle(OATheme.Colors.textPrimary)
                    .frame(maxWidth: .infinity, alignment: .center)
                    .padding(.vertical, 120)
            }
            .frame(maxWidth: .infinity)
        }
        .background(OATheme.Colors.background)
        .navigationTitle("")
        // Hard scroll edge effect (optional visual polish)
    }

    // No extra material; rely on OATheme surfaces
}
#endif
