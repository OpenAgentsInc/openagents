import SwiftUI

#if os(iOS)

struct Composer: UIViewRepresentable {
    @Binding var text: String
    var agentName: String
    var onSubmit: () -> Void
    private var placeholderString: String { "Message \(agentName)" }

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
        if #available(iOS 17.0, *) {
            textView.inlinePredictionType = .no
        }
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

        // Start empty; overlay placeholder label (not part of text)
        textView.text = text
        textView.textColor = .white

        let ph = UILabel()
        ph.text = placeholderString
        ph.font = UIFont(name: monoName, size: 16) ?? UIFont.monospacedSystemFont(ofSize: 16, weight: .regular)
        ph.textColor = .systemGray
        ph.numberOfLines = 1
        ph.isUserInteractionEnabled = false
        ph.translatesAutoresizingMaskIntoConstraints = false
        textView.addSubview(ph)
        NSLayoutConstraint.activate([
            ph.leadingAnchor.constraint(equalTo: textView.leadingAnchor, constant: 12),
            ph.topAnchor.constraint(equalTo: textView.topAnchor, constant: 10),
            ph.trailingAnchor.constraint(lessThanOrEqualTo: textView.trailingAnchor, constant: -12)
        ])
        ph.isHidden = !(text.isEmpty)
        context.coordinator.placeholder = ph

        return textView
    }

    func updateUIView(_ textView: UITextView, context: Context) {
        // Keep the text view synced with the binding and toggle placeholder
        if textView.text != text {
            textView.textColor = .white
            textView.text = text
        }
        if let ph = context.coordinator.placeholder {
            ph.text = placeholderString
            ph.isHidden = !(text.isEmpty) || textView.isFirstResponder
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
        weak var placeholder: UILabel?
        private var didPrime = false
        private var isPriming = false

        init(_ parent: Composer) {
            self.parent = parent
        }

        func textViewDidChange(_ textView: UITextView) {
        if !isPriming { parent.text = textView.text }
        placeholder?.isHidden = !textView.text.isEmpty
        }

        func textView(_ textView: UITextView, shouldChangeTextIn range: NSRange, replacementText text: String) -> Bool {
            if text == "\n" {
                parent.onSubmit()
                return false
            }
            return true
        }

        func textViewDidBeginEditing(_ textView: UITextView) { placeholder?.isHidden = true }
        func textViewDidEndEditing(_ textView: UITextView) { placeholder?.isHidden = !(parent.text.isEmpty) }
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
