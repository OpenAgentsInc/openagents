import SwiftUI

#if os(iOS)

struct Composer: UIViewRepresentable {
    @Binding var text: String
    var agentName: String
    var onSubmit: () -> Void

    func makeUIView(context: Context) -> UIView {
        let textView = UITextView()
        textView.text = ""
        textView.font = UIFont.systemFont(ofSize: 16)
        textView.backgroundColor = UIColor(white: 1.0, alpha: 0.1)
        textView.textColor = .white
        textView.tintColor = .white
        textView.returnKeyType = .send
        textView.enablesReturnKeyAutomatically = true
        textView.delegate = context.coordinator
        textView.isScrollEnabled = false
        textView.textContainerInset = UIEdgeInsets(top: 10, left: 12, bottom: 10, right: 12)
        textView.textContainer.lineFragmentPadding = 0

        // DISABLE HAPTIC FEEDBACK - this is what fixes the lag
        textView.autocorrectionType = .no
        textView.spellCheckingType = .no

        // Styling
        textView.layer.cornerRadius = 20
        textView.layer.masksToBounds = true

        // Set explicit height
        textView.translatesAutoresizingMaskIntoConstraints = false
        NSLayoutConstraint.activate([
            textView.heightAnchor.constraint(equalToConstant: 40)
        ])

        // Set placeholder
        if text.isEmpty {
            textView.text = "Message \(agentName)"
            textView.textColor = .systemGray
        }

        return textView
    }

    func updateUIView(_ uiView: UIView, context: Context) {
        guard let textView = uiView as? UITextView else { return }

        if text != textView.text {
            if text.isEmpty {
                textView.text = "Message \(agentName)"
                textView.textColor = .systemGray
            } else {
                if textView.textColor == .systemGray {
                    textView.textColor = .white
                }
                textView.text = text
            }
        }
    }

    func makeCoordinator() -> Coordinator {
        Coordinator(self)
    }

    class Coordinator: NSObject, UITextViewDelegate {
        var parent: Composer

        init(_ parent: Composer) {
            self.parent = parent
        }

        func textViewDidChange(_ textView: UITextView) {
            // Handle placeholder
            if textView.text.isEmpty {
                textView.text = "Message \(parent.agentName)"
                textView.textColor = .systemGray
                parent.text = ""
            } else if textView.textColor == .systemGray {
                textView.text = ""
                textView.textColor = .white
            } else {
                parent.text = textView.text
            }
        }

        func textView(_ textView: UITextView, shouldChangeTextIn range: NSRange, replacementText text: String) -> Bool {
            if text == "\n" {
                parent.onSubmit()
                return false
            }
            return true
        }

        func textViewDidBeginEditing(_ textView: UITextView) {
            if textView.textColor == .systemGray {
                textView.text = ""
                textView.textColor = .white
            }
        }

        func textViewDidEndEditing(_ textView: UITextView) {
            if textView.text.isEmpty {
                textView.text = "Message \(parent.agentName)"
                textView.textColor = .systemGray
            }
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
