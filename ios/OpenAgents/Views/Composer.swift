import SwiftUI

#if os(iOS)

struct Composer: UIViewRepresentable {
    @Binding var text: String
    var agentName: String
    var onSubmit: () -> Void

    func makeUIView(context: Context) -> UITextField {
        let textField = UITextField()
        textField.placeholder = "Message"
        textField.borderStyle = .roundedRect
        textField.backgroundColor = .systemGray6
        textField.returnKeyType = .send
        textField.enablesReturnKeyAutomatically = true
        textField.delegate = context.coordinator

        // DISABLE HAPTIC FEEDBACK
        textField.autocorrectionType = .no
        textField.spellCheckingType = .no

        return textField
    }

    func updateUIView(_ uiView: UITextField, context: Context) {
        uiView.text = text
    }

    func makeCoordinator() -> Coordinator {
        Coordinator(self)
    }

    class Coordinator: NSObject, UITextFieldDelegate {
        var parent: Composer

        init(_ parent: Composer) {
            self.parent = parent
        }

        func textFieldDidChangeSelection(_ textField: UITextField) {
            parent.text = textField.text ?? ""
        }

        func textFieldShouldReturn(_ textField: UITextField) -> Bool {
            parent.onSubmit()
            return true
        }
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
