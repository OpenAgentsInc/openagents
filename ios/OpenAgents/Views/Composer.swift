import SwiftUI

#if os(iOS)

struct Composer: UIViewRepresentable {
    @Binding var text: String
    var agentName: String
    var onSubmit: () -> Void

    func makeUIView(context: Context) -> UIView {
        let container = UIView()

        let textField = UITextField()
        textField.placeholder = "Message \(agentName)"
        textField.borderStyle = .none
        textField.backgroundColor = UIColor(white: 1.0, alpha: 0.1)
        textField.textColor = .white
        textField.tintColor = .white
        textField.returnKeyType = .send
        textField.enablesReturnKeyAutomatically = true
        textField.delegate = context.coordinator

        // DISABLE HAPTIC FEEDBACK - this is what fixes the lag
        textField.autocorrectionType = .no
        textField.spellCheckingType = .no

        // Styling
        textField.layer.cornerRadius = 24
        textField.layer.masksToBounds = true

        // Padding via left/right views
        let leftPadding = UIView(frame: CGRect(x: 0, y: 0, width: 16, height: 1))
        let rightPadding = UIView(frame: CGRect(x: 0, y: 0, width: 16, height: 1))
        textField.leftView = leftPadding
        textField.leftViewMode = .always
        textField.rightView = rightPadding
        textField.rightViewMode = .always

        textField.translatesAutoresizingMaskIntoConstraints = false
        container.addSubview(textField)

        NSLayoutConstraint.activate([
            textField.topAnchor.constraint(equalTo: container.topAnchor),
            textField.leadingAnchor.constraint(equalTo: container.leadingAnchor),
            textField.trailingAnchor.constraint(equalTo: container.trailingAnchor),
            textField.bottomAnchor.constraint(equalTo: container.bottomAnchor),
            textField.heightAnchor.constraint(greaterThanOrEqualToConstant: 44)
        ])

        return container
    }

    func updateUIView(_ uiView: UIView, context: Context) {
        guard let textField = uiView.subviews.first as? UITextField else { return }
        textField.text = text
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
