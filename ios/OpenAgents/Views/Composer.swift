import SwiftUI

#if os(iOS)

struct Composer: View {
    @Binding var text: String
    var agentName: String
    var onSubmit: () -> Void

    @FocusState private var isFocused: Bool

    var body: some View {
        HStack(spacing: 12) {
            TextField("Message \(agentName)", text: $text)
                .textFieldStyle(.plain)
                .font(.system(size: 16))
                .padding(12)
                .background(Color.gray.opacity(0.2))
                .cornerRadius(20)
                .focused($isFocused)

            Button(action: onSubmit) {
                Image(systemName: "arrow.up.circle.fill")
                    .font(.system(size: 32))
                    .foregroundColor(text.isEmpty ? .gray : .blue)
            }
            .disabled(text.isEmpty)
        }
        .padding()
        .background(.black)
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
