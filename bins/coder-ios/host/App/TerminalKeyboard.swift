// The terminal's native glue, shared by the Coder and OpenAgents hosts. Rust
// owns the session, the emulator, and every byte sent; this file supplies
// the keyboard target and lets the terminal rotate.
import SwiftUI
import UIKit

/// A keyboard target: typed text, Backspace, and hardware keys with their
/// modifiers go to Rust, which encodes them for the terminal.
struct TerminalKeyInput: UIViewRepresentable {
    @Binding var focused: Bool
    let text: (String) -> Void
    let key: (String, Bool, Bool, Bool) -> Void
    let paste: (String) -> Void

    func makeUIView(context: Context) -> TerminalKeyView {
        let view = TerminalKeyView()
        view.isAccessibilityElement = true
        view.accessibilityLabel = "Terminal input"
        view.accessibilityIdentifier = "terminal-input"
        return view
    }

    func updateUIView(_ view: TerminalKeyView, context: Context) {
        view.onText = text
        view.onKey = key
        view.onPaste = paste
        view.onResign = { if focused { focused = false } }
        if focused, !view.isFirstResponder {
            DispatchQueue.main.async { view.becomeFirstResponder() }
        } else if !focused, view.isFirstResponder {
            DispatchQueue.main.async { _ = view.resignFirstResponder() }
        }
    }
}

final class TerminalKeyView: UIView, UIKeyInput {
    var onText: (String) -> Void = { _ in }
    var onKey: (String, Bool, Bool, Bool) -> Void = { _, _, _, _ in }
    var onPaste: (String) -> Void = { _ in }
    var onResign: () -> Void = {}

    // Terminal input is exact: no correction, capitals, or smart punctuation.
    var autocorrectionType: UITextAutocorrectionType = .no
    var autocapitalizationType: UITextAutocapitalizationType = .none
    var spellCheckingType: UITextSpellCheckingType = .no
    var smartQuotesType: UITextSmartQuotesType = .no
    var smartDashesType: UITextSmartDashesType = .no
    var smartInsertDeleteType: UITextSmartInsertDeleteType = .no
    var keyboardType: UIKeyboardType = .asciiCapable
    var returnKeyType: UIReturnKeyType = .default

    override var canBecomeFirstResponder: Bool { true }
    // Always report text so Backspace reaches the shell on an empty line.
    var hasText: Bool { true }

    func insertText(_ text: String) { onText(text) }
    func deleteBackward() { onKey("backspace", false, false, false) }

    override func resignFirstResponder() -> Bool {
        let resigned = super.resignFirstResponder()
        if resigned { onResign() }
        return resigned
    }

    override func canPerformAction(_ action: Selector, withSender sender: Any?) -> Bool {
        action == #selector(paste(_:))
    }

    override func paste(_ sender: Any?) { onPaste(UIPasteboard.general.string ?? "") }

    override func pressesBegan(_ presses: Set<UIPress>, with event: UIPressesEvent?) {
        var unhandled = Set<UIPress>()
        for press in presses {
            guard let key = press.key, let name = Self.name(key) else {
                unhandled.insert(press)
                continue
            }
            let flags = key.modifierFlags
            onKey(name, flags.contains(.control), flags.contains(.alternate), flags.contains(.shift))
        }
        if !unhandled.isEmpty { super.pressesBegan(unhandled, with: event) }
    }

    /// The Rust key name for a hardware key that typed text doesn't carry:
    /// editing keys, and characters typed with Control or Option.
    private static func name(_ key: UIKey) -> String? {
        switch key.keyCode {
        case .keyboardUpArrow: return "up"
        case .keyboardDownArrow: return "down"
        case .keyboardLeftArrow: return "left"
        case .keyboardRightArrow: return "right"
        case .keyboardEscape: return "escape"
        case .keyboardTab: return key.modifierFlags.contains(.shift) ? "backtab" : "tab"
        case .keyboardHome: return "home"
        case .keyboardEnd: return "end"
        case .keyboardPageUp: return "page_up"
        case .keyboardPageDown: return "page_down"
        case .keyboardDeleteForward: return "delete"
        default:
            let flags = key.modifierFlags
            guard flags.contains(.control) || flags.contains(.alternate) else { return nil }
            let characters = key.charactersIgnoringModifiers
            return characters.count == 1 ? characters : nil
        }
    }
}

/// The app stays portrait; only an open terminal may rotate, so a wider grid
/// can use landscape.
enum TerminalOrientation {
    @MainActor static var mask: UIInterfaceOrientationMask = .portrait

    @MainActor static func allow(_ rotate: Bool) {
        mask = rotate ? .allButUpsideDown : .portrait
        for scene in UIApplication.shared.connectedScenes {
            guard let scene = scene as? UIWindowScene else { continue }
            scene.keyWindow?.rootViewController?.setNeedsUpdateOfSupportedInterfaceOrientations()
            if !rotate { scene.requestGeometryUpdate(.iOS(interfaceOrientations: .portrait)) }
        }
    }
}
