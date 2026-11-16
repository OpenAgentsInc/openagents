// Firm reason: 6 (AppKit text input over the shared GPU kit, including IME and secure fields).
import AppKit
import CoreText

/// SDK-owned active composition. The Coax edit buffer receives only committed whole edits.
final class KitMacEditor: NSTextView {
    var committed: ((String) -> Void)?
    var command: ((Selector) -> Bool)?
    var privateValue = false
    private var inputChange = 0
    private var holdingMarked = false
    init(style: [Float]) {
        // AppKit's designated initializer with a nil container creates no text
        // system (unlike UITextView). Supply the SDK's storage/layout/container.
        let storage = NSTextStorage(), layout = NSLayoutManager()
        let container = NSTextContainer(size: NSSize(width: 0, height: CGFloat.greatestFiniteMagnitude))
        storage.addLayoutManager(layout); layout.addTextContainer(container)
        super.init(frame: .zero, textContainer: container)
        setAccessibilityElement(true); setAccessibilityRole(.textArea)
        isVerticallyResizable = true; maxSize = NSSize(width: CGFloat.greatestFiniteMagnitude, height: CGFloat.greatestFiniteMagnitude)
        isRichText = false; drawsBackground = false
        isAutomaticQuoteSubstitutionEnabled = false; isAutomaticDashSubstitutionEnabled = false
        isAutomaticSpellingCorrectionEnabled = false; isContinuousSpellCheckingEnabled = false
        textContainerInset = .zero; textContainer?.lineFragmentPadding = 0
        textContainer?.widthTracksTextView = true
        let size = CGFloat(style[0])
        if let path = Bundle.main.path(forResource: "Geist", ofType: "ttf", inDirectory: "Components/fonts"), let provider = CGDataProvider(url: URL(fileURLWithPath: path) as CFURL), let face = CGFont(provider) {
            font = CTFontCreateWithGraphicsFont(face, size, nil, nil) as NSFont
        } else { font = .systemFont(ofSize: size) }
        textColor = NSColor(red: CGFloat(style[2]), green: CGFloat(style[3]), blue: CGFloat(style[4]), alpha: CGFloat(style[5]))
        insertionPointColor = NSColor(red: CGFloat(style[10]), green: CGFloat(style[11]), blue: CGFloat(style[12]), alpha: CGFloat(style[13]))
        let paragraph = NSMutableParagraphStyle(); paragraph.minimumLineHeight = CGFloat(style[1]); paragraph.maximumLineHeight = CGFloat(style[1])
        defaultParagraphStyle = paragraph
    }
    required init?(coder: NSCoder) { fatalError() }
    private func publish() { if inputChange == 0 && !holdingMarked && !hasMarkedText() { committed?(string) } }
    /// Programmatic synchronization and retirement are not edits from the user.
    func synchronize(_ apply: () -> Void) { inputChange += 1; defer { inputChange -= 1 }; apply() }
    func discardComposition() { synchronize { super.unmarkText() }; holdingMarked = false }
    override func didChangeText() { super.didChangeText(); publish() }
    // AppKit can notify didChangeText before updating its marked range. Guard
    // the whole SDK operation, not just the range seen inside that notification.
    override func setMarkedText(_ value: Any, selectedRange: NSRange, replacementRange: NSRange) {
        holdingMarked = !(value as? String ?? (value as? NSAttributedString)?.string ?? "").isEmpty
        inputChange += 1; super.setMarkedText(value, selectedRange: selectedRange, replacementRange: replacementRange); inputChange -= 1
    }
    override func insertText(_ value: Any, replacementRange: NSRange) {
        inputChange += 1; super.insertText(value, replacementRange: replacementRange); inputChange -= 1; holdingMarked = false; publish()
    }
    override func unmarkText() { let outer = inputChange == 0; inputChange += 1; super.unmarkText(); inputChange -= 1; if outer { holdingMarked = false }; publish() }
    override func doCommand(by selector: Selector) { if command?(selector) != true { super.doCommand(by: selector) } }
    override func accessibilityValue() -> String? { privateValue ? nil : super.accessibilityValue() }
    override func copy(_ sender: Any?) {
        guard privateValue else { super.copy(sender); return }
        let range = selectedRange(), text = string as NSString
        guard range.location != NSNotFound, NSMaxRange(range) <= text.length, range.length > 0 else { return }
        let board = NSPasteboard.general; board.clearContents(); board.setString(text.substring(with: range), forType: .string)
        board.setData(Data(), forType: .init("org.nspasteboard.ConcealedType"))
        let revision = board.changeCount
        DispatchQueue.main.asyncAfter(deadline: .now() + 60) { if board.changeCount == revision { board.clearContents() } }
    }
}
final class KitMacSecure: NSSecureTextField {
    override init(frame: NSRect) { super.init(frame: frame); setAccessibilityElement(true); setAccessibilityRole(.textField) }
    required init?(coder: NSCoder) { fatalError() }
    override func accessibilityValue() -> String? { nil }
}
