import SwiftUI

#if os(iOS)

struct Composer: View {
    @Binding var text: String
    var agentName: String
    var onSubmit: () -> Void

    var body: some View {
        HStack {
            TextField("Message", text: $text)
                .textFieldStyle(.plain)
                .padding(12)
                .background(Color.gray.opacity(0.2))
                .cornerRadius(20)

            Button(action: onSubmit) {
                Image(systemName: "arrow.up.circle.fill")
                    .font(.system(size: 32))
                    .foregroundColor(.blue)
            }
        }
        .padding()
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
