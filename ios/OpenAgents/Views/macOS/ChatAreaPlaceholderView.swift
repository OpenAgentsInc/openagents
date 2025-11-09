import SwiftUI

#if os(macOS)
struct ChatAreaPlaceholderView: View {
    var body: some View {
        ZStack {
            OATheme.Colors.background
                .ignoresSafeArea()
            Text("Hello")
                .font(OAFonts.ui(.title, 48))
                .foregroundStyle(OATheme.Colors.textPrimary)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .navigationTitle("")
    }

    // No extra material; rely on OATheme surfaces
}
#endif
