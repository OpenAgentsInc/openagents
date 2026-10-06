// The world computer's screen is drawn, laid out, and hit-tested by Rust in
// the Verse HUD. This file only mirrors its laid-out controls for
// accessibility and supplies the two native pieces a HUD cannot: the camera
// scanner and the keyboard for the current input request.
import SwiftUI
import UIKit

struct VerseComputerHud: Decodable {
    let visible: Bool
    let page: String
    let frame: [Double]
    let body: [Double]
    let items: [VerseHudItem]
    let input: ComputersInput?
    let busy: Bool
    let scroll: Double
    let max_scroll: Double
    let captured_pointers: [UInt64]

    var valid: Bool {
        ["computers", "terminal", "chats"].contains(page) && VerseDoors.validFrame(frame) &&
        VerseDoors.validFrame(body) && items.count <= 160 && items.allSatisfy(\.valid) &&
        scroll.isFinite && max_scroll.isFinite && captured_pointers.count <= 8
    }
    var observation: [String: Any] {
        ["visible": visible, "page": page, "busy": busy, "scroll": scroll, "max_scroll": max_scroll,
         "body": body, "input": input?.token ?? "",
         "items": items.map { ["key": $0.key, "label": $0.label, "role": $0.role,
                               "enabled": $0.enabled, "frame": $0.frame] as [String: Any] }]
    }
}

struct VerseHudItem: Decodable, Equatable {
    let key: String
    let label: String
    let role: String
    let enabled: Bool
    let frame: [Double]

    var valid: Bool {
        !key.isEmpty && key.utf8.count <= 256 && label.utf8.count <= 2048 &&
        ["button", "heading", "text"].contains(role) && VerseDoors.validFrame(frame)
    }
    var rect: CGRect { CGRect(x: frame[0], y: frame[1], width: frame[2], height: frame[3]) }
}

/// What the HUD asks the native host to do, once each.
struct VerseComputerCommand: Decodable {
    let kind: String
    let text: String?
    let surface: String?
    let instance: String?
    let revision: UInt64?
    let node: String?
    let token: String?
    /// The grid a `terminal_resize` command asks for.
    let rows: Int?
    let cols: Int?
}

/// Accessibility for the HUD: one element per laid-out control, over the
/// world surface. It takes no touches; activating an element asks Rust to
/// run the control with that key.
struct ComputerHudAccessibility: UIViewRepresentable {
    let items: [VerseHudItem]
    let activate: (String) -> Void
    let scroll: (Double) -> Void

    func makeUIView(context: Context) -> HudAccessibilityView {
        let view = HudAccessibilityView()
        view.isUserInteractionEnabled = false
        view.backgroundColor = .clear
        return view
    }

    func updateUIView(_ view: HudAccessibilityView, context: Context) {
        view.update(items: items, activate: activate, scroll: scroll)
    }
}

final class HudAccessibilityView: UIView {
    private var shown: [VerseHudItem] = []
    private var elements: [HudElement] = []

    func update(items: [VerseHudItem], activate: @escaping (String) -> Void,
                scroll: @escaping (Double) -> Void) {
        guard items != shown else { return }
        shown = items
        elements = items.map { item in
            let element = HudElement(accessibilityContainer: self)
            element.accessibilityIdentifier = item.key
            element.accessibilityLabel = item.label
            element.accessibilityFrameInContainerSpace = item.rect
            switch item.role {
            case "button":
                element.accessibilityTraits = item.enabled ? .button : [.button, .notEnabled]
            case "heading": element.accessibilityTraits = [.header, .staticText]
            default: element.accessibilityTraits = .staticText
            }
            element.activate = { activate(item.key) }
            element.scroll = scroll
            return element
        }
        accessibilityElements = elements
        UIAccessibility.post(notification: .layoutChanged, argument: nil)
    }
}

final class HudElement: UIAccessibilityElement {
    var activate: () -> Void = {}
    var scroll: (Double) -> Void = { _ in }

    override func accessibilityActivate() -> Bool {
        guard accessibilityTraits.contains(.button), !accessibilityTraits.contains(.notEnabled) else { return false }
        activate()
        return true
    }

    override func accessibilityScroll(_ direction: UIAccessibilityScrollDirection) -> Bool {
        switch direction {
        case .up: scroll(-240)
        case .down: scroll(240)
        default: return false
        }
        return true
    }
}

/// The keyboard or camera for the Computers surface's current input
/// request. Rust validates every value; nothing here is kept.
struct ComputerInputBar: View {
    let input: ComputersInput
    let scanning: Bool
    let busy: Bool
    let submit: (String) -> Void
    let cancel: () -> Void
    let stopScanning: () -> Void
    @State private var value = ""
    @State private var inputError: String?
    @FocusState private var focused: Bool

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text(input.label).font(.headline)
            Text(input.prompt).font(.caption)
            if let inputError {
                Text(inputError).font(.caption).accessibilityIdentifier("computers-input-error")
            }
            if scanning {
                InlineQRScanner { scanned in send(scanned) }
                Button("Type instead") { stopScanning() }
                    .accessibilityIdentifier("computers-scan-stop")
            } else if input.secret {
                SecretInputField(label: input.label, value: $value) { if !busy { send(value) } }
                    .textContentType(nil)
            } else {
                TextField(input.label, text: $value, axis: .vertical)
                    .lineLimit(1...6)
                    .focused($focused)
                    .autocorrectionDisabled().textInputAutocapitalization(.never)
                    .accessibilityLabel(input.label).accessibilityIdentifier("computers-input")
            }
            HStack {
                if !scanning {
                    Button("Submit") { send(value) }
                        .disabled(busy || value.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
                        .accessibilityIdentifier("computers-submit")
                }
                Button("Cancel") { value = ""; cancel() }
                    .accessibilityIdentifier("computers-cancel")
            }
        }
        .padding(12)
        .background(Color(red: 0.025, green: 0.02, blue: 0).opacity(0.97), in: RoundedRectangle(cornerRadius: 16))
        .overlay(RoundedRectangle(cornerRadius: 16).stroke(.tint.opacity(0.7), lineWidth: 1))
        .onAppear { value = ""; focused = !scanning && !input.secret }
        .onDisappear { value = "" }
    }

    private func send(_ text: String) {
        UIApplication.shared.sendAction(#selector(UIResponder.resignFirstResponder), to: nil, from: nil, for: nil)
        guard text.utf8.count <= input.max_bytes else {
            inputError = "That's too long. Copy it again."
            return
        }
        inputError = nil
        value = ""
        submit(text)
    }
}
