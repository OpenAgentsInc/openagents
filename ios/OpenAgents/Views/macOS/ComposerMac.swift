import SwiftUI
#if os(macOS)
import AppKit
#endif

#if os(macOS)
struct ComposerMac: View {
    @Binding var text: String
    var placeholder: String = "Ask OpenAgents"
    var isSending: Bool = false
    var onSend: () -> Void

    @State private var measuredHeight: CGFloat = 36

    var body: some View {
        HStack(alignment: .bottom, spacing: 12) {
            ZStack(alignment: .topLeading) {
                NSTextViewWrapper(
                    text: $text,
                    dynamicHeight: $measuredHeight,
                    fontSize: 14,
                    onReturn: handleReturn
                )
                .frame(minHeight: 36, maxHeight: 144)
                .frame(height: min(max(measuredHeight, 36), 144))

                if text.isEmpty {
                    Text(placeholder)
                        .font(OAFonts.mono(.body, 14))
                        .foregroundStyle(OATheme.Colors.textSecondary)
                        .padding(.leading, 18)
                        .padding(.top, 10)
                        .allowsHitTesting(false)
                }
            }

            Button(action: handleReturn) {
                Image(systemName: "arrow.up")
                    .symbolRenderingMode(.monochrome)
                    .foregroundStyle(.white)
                    .font(.system(size: 16, weight: .semibold))
                    .frame(width: 36, height: 36)
            }
            .buttonStyle(.plain)
            .background(Capsule().fill(OATheme.Colors.background))
            .overlay(
                Capsule().strokeBorder(OATheme.Colors.textSecondary.opacity(0.3), lineWidth: 0.5)
            )
            .clipShape(Capsule())
            .shadow(color: Color.black.opacity(0.25), radius: 8, x: 0, y: 4)
            .opacity((text.isEmpty || isSending) ? 0.5 : 1.0)
            .disabled(text.isEmpty || isSending)
        }
        .padding(12)
        .background(
            RoundedRectangle(cornerRadius: 12, style: .continuous)
                .fill(OATheme.Colors.sidebarBackground) // dark gray theme surface
        )
    }

    private func handleReturn() {
        guard !text.isEmpty, !isSending else { return }
        onSend()
    }
}

// MARK: - NSTextView wrapper with dynamic height and return handling
private struct NSTextViewWrapper: NSViewRepresentable {
    @Binding var text: String
    @Binding var dynamicHeight: CGFloat
    var fontSize: CGFloat
    var onReturn: () -> Void

    func makeNSView(context: Context) -> NSScrollView {
        let scrollView = NSTextView.scrollableTextView()
        scrollView.borderType = .noBorder
        scrollView.drawsBackground = false
        scrollView.hasVerticalScroller = false
        scrollView.hasHorizontalScroller = false

        let tv = scrollView.documentView as! NSTextView
        tv.delegate = context.coordinator
        tv.drawsBackground = false
        tv.isRichText = false
        tv.isAutomaticQuoteSubstitutionEnabled = false
        tv.isAutomaticDashSubstitutionEnabled = false
        tv.isAutomaticTextReplacementEnabled = false
        tv.isAutomaticSpellingCorrectionEnabled = false
        tv.textColor = NSColor.white
        tv.insertionPointColor = NSColor.white
        if let berkeley = NSFont(name: BerkeleyFont.defaultName(), size: fontSize) {
            tv.font = berkeley
        } else {
            tv.font = NSFont.monospacedSystemFont(ofSize: fontSize, weight: .regular)
        }
        tv.textContainerInset = NSSize(width: 10, height: 8)
        tv.textContainer?.lineFragmentPadding = 8

        tv.isVerticallyResizable = true
        tv.isHorizontallyResizable = false
        tv.textContainer?.widthTracksTextView = true
        tv.textContainer?.containerSize = NSSize(width: scrollView.contentSize.width, height: .greatestFiniteMagnitude)

        // initial measure
        DispatchQueue.main.async { self.recalculateHeight(view: tv) }
        return scrollView
    }

    func updateNSView(_ nsView: NSScrollView, context: Context) {
        let tv = nsView.documentView as! NSTextView
        if tv.string != text { tv.string = text }
        tv.textContainer?.containerSize = NSSize(width: nsView.contentSize.width, height: .greatestFiniteMagnitude)
        // Defer measurement to avoid triggering layout recursion inside SwiftUI updates
        DispatchQueue.main.async { self.recalculateHeight(view: tv) }
    }

    func makeCoordinator() -> Coordinator { Coordinator(self) }

    private func recalculateHeight(view: NSTextView) {
        // Always perform measurement outside the current layout pass.
        DispatchQueue.main.async {
            guard let container = view.textContainer, let lm = view.layoutManager else { return }
            lm.ensureLayout(for: container)
            let used = lm.usedRect(for: container)
            let inset = view.textContainerInset.height * 2
            let newHeight = used.height + inset
            let clamped = min(max(newHeight, 36), 144)
            if abs(self.dynamicHeight - clamped) > 0.5 {
                self.dynamicHeight = clamped
            }
        }
    }

    final class Coordinator: NSObject, NSTextViewDelegate {
        private let parent: NSTextViewWrapper
        init(_ parent: NSTextViewWrapper) { self.parent = parent }

        func textDidChange(_ notification: Notification) {
            guard let tv = notification.object as? NSTextView else { return }
            parent.text = tv.string
            // Defer height calc to avoid layout recursion during text edits
            DispatchQueue.main.async { self.parent.recalculateHeight(view: tv) }
        }

        func textView(_ textView: NSTextView, doCommandBy commandSelector: Selector) -> Bool {
            if commandSelector == #selector(NSResponder.insertNewline(_:)) {
                if NSApp.currentEvent?.modifierFlags.contains(.shift) == true {
                    return false // allow newline
                }
                parent.onReturn()
                return true
            }
            return false
        }
    }
}
#endif
