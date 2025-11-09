import SwiftUI

#if os(macOS)
struct ChatAreaPlaceholderView: View {
    @State private var fadeIn: Bool = false
    @EnvironmentObject private var bridge: BridgeManager
    @State private var inputText: String = ""
    @State private var isSending: Bool = false
    var body: some View {
        VStack(spacing: 0) {
            ZStack {
                OATheme.Colors.background
                    .ignoresSafeArea()
                Text("Hello")
                    .font(OAFonts.ui(.title, 48))
                    .foregroundStyle(OATheme.Colors.textPrimary)
                    .opacity(fadeIn ? 1.0 : 0.0)
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)

            // Composer anchored at bottom (non-scrolling main area)
            HStack {
                ComposerMac(text: $inputText, isSending: isSending) {
                    send()
                }
            }
            .padding(.horizontal, 16)
            .padding(.vertical, 12)
            .background(OATheme.Colors.background)
        }
        .navigationTitle("")
        .onAppear {
            withAnimation(.easeIn(duration: 1.8)) { fadeIn = true }
        }
    }

    // No extra material; rely on OATheme surfaces

    private func send() {
        let trimmed = inputText.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return }
        isSending = true
        let old = inputText
        inputText = ""
        bridge.dispatcher?.sendPrompt(
            text: trimmed,
            desiredMode: nil,
            getSessionId: { bridge.currentSessionId },
            setSessionId: { bridge.currentSessionId = $0 }
        )
        // Optimistically end sending; future: bind to timeline/updates for busy state
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.05) {
            isSending = false
        }
        // In case dispatcher is nil, restore text
        if bridge.dispatcher == nil { inputText = old; isSending = false }
    }
}
#endif
