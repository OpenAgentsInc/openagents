import SwiftUI

#if os(macOS)
struct ChatAreaPlaceholderView: View {
    @State private var fadeIn: Bool = false
    var body: some View {
        ZStack {
            OATheme.Colors.background
                .ignoresSafeArea()
            Text("Hello")
                .font(OAFonts.ui(.title, 48))
                .foregroundStyle(OATheme.Colors.textPrimary)
                .opacity(fadeIn ? 1.0 : 0.0)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .navigationTitle("")
        .onAppear {
            withAnimation(.easeIn(duration: 1.8)) {
                fadeIn = true
            }
        }
    }

    // No extra material; rely on OATheme surfaces
}
#endif
