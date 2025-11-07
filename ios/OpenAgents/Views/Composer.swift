import SwiftUI

#if os(iOS)

struct Composer: UIViewRepresentable {
    @Binding var text: String
    var agentName: String
    var onSubmit: () -> Void

    func makeUIView(context: Context) -> UITextView {
        let textView = UITextView()
        textView.text = ""
        // Use our mono font (Berkeley Mono)
        let monoName = BerkeleyFont.defaultName()
        textView.font = UIFont(name: monoName, size: 16) ?? UIFont.monospacedSystemFont(ofSize: 16, weight: .regular)
        textView.backgroundColor = UIColor(white: 1.0, alpha: 0.1)
        textView.textColor = .white
        textView.tintColor = .white
        textView.returnKeyType = .send
        textView.keyboardType = .asciiCapable
        textView.keyboardAppearance = .dark
        textView.enablesReturnKeyAutomatically = true
        textView.delegate = context.coordinator
        textView.isScrollEnabled = false
        textView.textContainerInset = UIEdgeInsets(top: 10, left: 12, bottom: 10, right: 12)
        textView.textContainer.lineFragmentPadding = 0

        // Disable all smart features, correction, and spell checking to prevent
        // haptic/prediction overhead and focus lag.
        textView.autocorrectionType = .no
        textView.spellCheckingType = .no
        textView.autocapitalizationType = .none
        textView.smartDashesType = .no
        textView.smartQuotesType = .no
        textView.smartInsertDeleteType = .no
        textView.dataDetectorTypes = []
        textView.keyboardDismissMode = .interactive
        // Hide the QuickType bar (input assistant) to reduce layout churn
        let ia = textView.inputAssistantItem
        ia.leadingBarButtonGroups = []
        ia.trailingBarButtonGroups = []
        // Avoid text drag interaction overhead
        if let drag = textView.textDragInteraction { drag.isEnabled = false }
        textView.allowsEditingTextAttributes = false

        // Styling
        textView.layer.cornerRadius = 20
        textView.layer.masksToBounds = true

        // Expand to fill available horizontal space inside HStack
        textView.setContentHuggingPriority(.defaultLow, for: .horizontal)
        textView.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)
        textView.setContentHuggingPriority(.defaultHigh, for: .vertical)
        textView.setContentCompressionResistancePriority(.required, for: .vertical)

        // Start empty; no placeholder text to avoid state churn
        textView.text = text
        textView.textColor = .white
        
        return textView
    }

    func updateUIView(_ textView: UITextView, context: Context) {
        // Keep the text view synced with the binding without placeholder logic
        if textView.text != text {
            textView.textColor = .white
            textView.text = text
        }
    }

    // Ensure the SwiftUI layout engine gets a deterministic size
    func sizeThatFits(_ proposal: ProposedViewSize, uiView: UITextView, context: Context) -> CGSize {
        let width = proposal.width ?? 0
        return CGSize(width: width, height: 40)
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
            parent.text = textView.text
        }

        func textView(_ textView: UITextView, shouldChangeTextIn range: NSRange, replacementText text: String) -> Bool {
            if text == "\n" {
                parent.onSubmit()
                return false
            }
            return true
        }

        func textViewDidBeginEditing(_ textView: UITextView) {}
        func textViewDidEndEditing(_ textView: UITextView) {}
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
        .frame(maxWidth: .infinity)
    }
    .background(OATheme.Colors.background)
    .preferredColorScheme(.dark)
}

#endif
