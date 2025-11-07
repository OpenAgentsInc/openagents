import SwiftUI

#if os(iOS)

struct Composer: View {
    @Binding var text: String
    var agentName: String
    var onSubmit: () -> Void

    @FocusState private var isFocused: Bool

    private var isEmpty: Bool {
        text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
    }

    var body: some View {
        HStack(spacing: 12) {
            // Text field
            TextField("Message \(agentName)", text: $text, axis: .vertical)
                .font(.system(size: 16))
                .foregroundStyle(.primary)
                .padding(.horizontal, 16)
                .padding(.vertical, 12)
                .background(
                    RoundedRectangle(cornerRadius: 24, style: .continuous)
                        .fill(Color.white.opacity(0.1))
                )
                .focused($isFocused)
                .submitLabel(.send)
                .onSubmit {
                    if !isEmpty { onSubmit() }
                }

            // Submit button
            Button(action: {
                if !isEmpty { onSubmit() }
            }) {
                Image(systemName: "arrow.up.circle.fill")
                    .font(.system(size: 32))
                    .foregroundStyle(isEmpty ? Color.gray : Color.blue)
            }
            .disabled(isEmpty)
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 12)
        .background(Color.black)
    }
}

#Preview {
    VStack {
        Spacer()
        Composer(
            text: .constant(""),
            agentName: "Codex",
            onSubmit: {}
        )
    }
    .background(OATheme.Colors.background)
    .preferredColorScheme(.dark)
}

#endif
