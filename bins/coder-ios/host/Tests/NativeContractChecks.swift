// Deterministic decoder checks. This executable mounts no native application.
import Foundation

@main
struct NativeContractChecks {
    static func main() throws {
        let source = "# café 日本語 👩🏽‍💻\n\n<script>literal</script>\n[link](https://example.invalid)"
        let text: [String: Any] = [
            "key": "event-1", "style": [:],
            "element": ["kind": "text", "props": ["value": source, "role": "markdown"]],
        ]
        let button: [String: Any] = [
            "key": "more", "style": ["foreground": ["red": 1, "green": 2, "blue": 3, "alpha": 255]],
            "element": ["kind": "button", "props": ["label": "Earlier messages", "enabled": false,
                                                       "intent": ["opaque_application_intent": "ignored by native"]]],
        ]
        var fixture: [String: Any] = [
            "schema": "rust-native.view.v1", "instance": "synthetic", "revision": 2,
            "root": ["key": "timeline", "style": [:], "element": ["kind": "list", "props": [
                "label": "Synthetic timeline", "children": [text, button],
            ]]],
        ]
        let decoded = try JSONDecoder().decode(NativeView.self, from: JSONSerialization.data(withJSONObject: fixture))
        guard decoded.instance == "synthetic", decoded.revision == 2,
              case let .list(label, rows) = decoded.root.element,
              label == "Synthetic timeline", rows.map(\.key) == ["event-1", "more"],
              case let .text(value, role) = rows[0].element, value == source, role == "markdown",
              case let .button(_, enabled) = rows[1].element, !enabled,
              rows[1].style.foreground?.red == 1 else {
            throw Failure.contract
        }
        fixture["root"] = ["key": "unknown", "style": [:],
                            "element": ["kind": "remote_script", "props": [:]]]
        do {
            _ = try JSONDecoder().decode(NativeView.self, from: JSONSerialization.data(withJSONObject: fixture))
            throw Failure.contract
        } catch is DecodingError {}
        print("Native contract checks passed: list identity, exact Markdown, opaque intent, disabled state, generic RGBA, unsupported element refusal.")
    }
    enum Failure: Error { case contract }
}
