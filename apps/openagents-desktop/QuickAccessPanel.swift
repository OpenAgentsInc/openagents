import AppKit

/// Global Quick Access floating NSPanel (#14146): summoned from any macOS app via double-tap Option.
/// Styled in OpenAgents' pure-black/charcoal #111111 surface with crisp 1px #262626 border,
/// nonactivatingPanel + canJoinAllSpaces + NSScreen.visibleFrame clamping.
final class QuickAccessController: NSObject, NSTextFieldDelegate {
    static let shared = QuickAccessController()

    private var panel: NSPanel?
    private var field: NSTextField?
    private var lastOptionRelease: TimeInterval = 0
    private var optionWasDown = false
    private var globalMonitor: Any?
    private var localMonitor: Any?
    var onSubmit: ((String) -> Void)?

    func startMonitoring() {
        guard globalMonitor == nil else { return }
        globalMonitor = NSEvent.addGlobalMonitorForEvents(matching: .flagsChanged) { [weak self] ev in
            self?.handleFlagsChanged(ev)
        }
        localMonitor = NSEvent.addLocalMonitorForEvents(matching: .flagsChanged) { [weak self] ev in
            self?.handleFlagsChanged(ev)
            return ev
        }
    }

    func stopMonitoring() {
        if let g = globalMonitor { NSEvent.removeMonitor(g); globalMonitor = nil }
        if let l = localMonitor { NSEvent.removeMonitor(l); localMonitor = nil }
    }

    private func handleFlagsChanged(_ ev: NSEvent) {
        let mods = ev.modifierFlags.intersection(.deviceIndependentFlagsMask)
        let onlyOption = mods == .option
        if onlyOption {
            optionWasDown = true
        } else if optionWasDown && mods.isEmpty {
            optionWasDown = false
            let now = ProcessInfo.processInfo.systemUptime
            if now - lastOptionRelease < 0.35 {
                lastOptionRelease = 0
                DispatchQueue.main.async { [weak self] in self?.toggle() }
            } else {
                lastOptionRelease = now
            }
        } else {
            optionWasDown = false
        }
    }

    func toggle() {
        if let p = panel, p.isVisible {
            p.orderOut(nil)
            return
        }
        show()
    }

    func show() {
        let p = ensurePanel()
        if let screen = NSScreen.main ?? NSScreen.screens.first {
            let vf = screen.visibleFrame
            let w: CGFloat = 620
            let h: CGFloat = 68
            var x = vf.midX - w / 2
            var y = vf.midY + 120
            // Clamp inside NSScreen.visibleFrame (Skooching).
            x = max(vf.minX + 16, min(x, vf.maxX - w - 16))
            y = max(vf.minY + 16, min(y, vf.maxY - h - 16))
            p.setFrame(NSRect(x: x, y: y, width: w, height: h), display: true)
        }
        field?.stringValue = ""
        p.makeKeyAndOrderFront(nil)
        NSApp.activate(ignoringOtherApps: true)
        if let f = field { p.makeFirstResponder(f) }
    }

    private func ensurePanel() -> NSPanel {
        if let existing = panel { return existing }
        let p = NSPanel(
            contentRect: NSRect(x: 0, y: 0, width: 620, height: 68),
            styleMask: [.borderless, .nonactivatingPanel],
            backing: .buffered,
            defer: false
        )
        p.isFloatingPanel = true
        p.level = .floating
        p.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary]
        p.backgroundColor = .clear
        p.isOpaque = false
        p.hasShadow = true

        let root = NSView(frame: NSRect(x: 0, y: 0, width: 620, height: 68))
        root.wantsLayer = true
        root.layer?.backgroundColor = NSColor(srgbRed: 0x11 / 255.0, green: 0x11 / 255.0, blue: 0x11 / 255.0, alpha: 0.98).cgColor
        root.layer?.cornerRadius = 18
        root.layer?.borderWidth = 1
        root.layer?.borderColor = NSColor(srgbRed: 0x26 / 255.0, green: 0x26 / 255.0, blue: 0x26 / 255.0, alpha: 1.0).cgColor

        let tf = NSTextField(frame: NSRect(x: 18, y: 18, width: 584, height: 32))
        tf.placeholderString = "Ask OpenAgents anything… (Return to send, Esc to dismiss)"
        tf.isBordered = false
        tf.drawsBackground = false
        tf.focusRingType = .none
        tf.textColor = NSColor(srgbRed: 0xf5 / 255.0, green: 0xf5 / 255.0, blue: 0xf5 / 255.0, alpha: 1.0)
        tf.font = NSFont.systemFont(ofSize: 15, weight: .regular)
        tf.delegate = self
        root.addSubview(tf)
        p.contentView = root
        self.panel = p
        self.field = tf
        return p
    }

    func control(_ control: NSControl, textView: NSTextView, doCommandBy commandSelector: Selector) -> Bool {
        if commandSelector == #selector(NSResponder.cancelOperation(_:)) {
            panel?.orderOut(nil)
            return true
        }
        if commandSelector == #selector(NSResponder.insertNewline(_:)) {
            let text = field?.stringValue.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
            panel?.orderOut(nil)
            if !text.isEmpty { onSubmit?(text) }
            return true
        }
        return false
    }
}
