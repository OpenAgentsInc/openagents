import AppKit

/// Rich macOS Menu Bar NSPopover (#14146) replacing the bare 2-item NSMenu.
/// Displays live status, quick New Chat / Quick Access actions, and Show/Quit controls in #111111 skin.
final class TrayPopoverController: NSViewController {
    var onShowWindow: (() -> Void)?
    var onNewChat: (() -> Void)?
    var onQuickAccess: (() -> Void)?
    var onQuit: (() -> Void)?
    private var statusLabel: NSTextField?

    override func loadView() {
        let root = NSView(frame: NSRect(x: 0, y: 0, width: 260, height: 188))
        root.wantsLayer = true
        root.layer?.backgroundColor = NSColor(srgbRed: 0x11 / 255.0, green: 0x11 / 255.0, blue: 0x11 / 255.0, alpha: 1.0).cgColor

        let title = NSTextField(labelWithString: "OpenAgents")
        title.font = NSFont.systemFont(ofSize: 13, weight: .semibold)
        title.textColor = NSColor(srgbRed: 0xf5 / 255.0, green: 0xf5 / 255.0, blue: 0xf5 / 255.0, alpha: 1.0)
        title.frame = NSRect(x: 14, y: 156, width: 232, height: 18)
        root.addSubview(title)

        let st = NSTextField(labelWithString: "Daemon ready · Tap ⌥ twice for Quick Access")
        st.font = NSFont.systemFont(ofSize: 11, weight: .regular)
        st.textColor = NSColor(srgbRed: 0x81 / 255.0, green: 0x81 / 255.0, blue: 0x81 / 255.0, alpha: 1.0)
        st.frame = NSRect(x: 14, y: 136, width: 232, height: 16)
        root.addSubview(st)
        self.statusLabel = st

        let buttons: [(String, Selector, CGFloat)] = [
            ("New Chat", #selector(tapNewChat), 96),
            ("Quick Access (⌥×2)", #selector(tapQuickAccess), 66),
            ("Show OpenAgents", #selector(tapShow), 36),
            ("Quit OpenAgents", #selector(tapQuit), 8),
        ]
        for (label, sel, y) in buttons {
            let btn = NSButton(title: label, target: self, action: sel)
            btn.bezelStyle = .rounded
            btn.frame = NSRect(x: 12, y: y, width: 236, height: 26)
            root.addSubview(btn)
        }
        self.view = root
    }

    func updateStatus(_ text: String) {
        statusLabel?.stringValue = text
    }

    @objc private func tapNewChat() { onNewChat?() }
    @objc private func tapQuickAccess() { onQuickAccess?() }
    @objc private func tapShow() { onShowWindow?() }
    @objc private func tapQuit() { onQuit?() }
}
