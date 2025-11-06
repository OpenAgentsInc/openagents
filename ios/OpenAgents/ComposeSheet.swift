import SwiftUI

#if os(iOS)
struct ComposeSheet: View {
    @EnvironmentObject var bridge: BridgeManager
    @Environment(\.dismiss) private var dismiss

    @State private var messageText: String = ""
    @FocusState private var isTextFieldFocused: Bool

    var body: some View {
        NavigationView {
            VStack(spacing: 0) {
                // Text input area
                TextEditor(text: $messageText)
                    .font(OAFonts.ui(.body, 16))
                    .foregroundStyle(OATheme.Colors.textPrimary)
                    .padding(.horizontal, 16)
                    .padding(.vertical, 12)
                    .focused($isTextFieldFocused)
                    .scrollContentBackground(.hidden)
                    .background(OATheme.Colors.background)

                Spacer()
            }
            .background(OATheme.Colors.background)
            .navigationTitle("New Message")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel") {
                        dismiss()
                    }
                    .foregroundStyle(OATheme.Colors.textSecondary)
                }

                ToolbarItem(placement: .confirmationAction) {
                    Button("Send") {
                        sendMessage()
                    }
                    .disabled(messageText.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
                    .foregroundStyle(
                        messageText.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
                            ? OATheme.Colors.textSecondary.opacity(0.5)
                            : OATheme.Colors.accent
                    )
                }
            }
        }
        .onAppear {
            // Auto-focus the text field when sheet appears
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.5) {
                isTextFieldFocused = true
            }
        }
    }

    private func sendMessage() {
        let trimmed = messageText.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return }

        bridge.sendPrompt(text: trimmed)
        dismiss()
    }
}

#if DEBUG
struct ComposeSheet_Previews: PreviewProvider {
    static var previews: some View {
        ComposeSheet()
            .environmentObject(BridgeManager())
    }
}
#endif
#endif
